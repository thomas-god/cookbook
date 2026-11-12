use std::time::SystemTime;

use cookbook_domain::recipe::models::{
    NewRecipeSourceDocument, RecipeCategory, RecipeId as RecipeIdValue,
    RecipeImageId as RecipeImageIdValue, RecipeRevision, RecipeRevisionId, RecipeScope,
    RecipeSourceDocumentId as RecipeSourceDocumentIdValue, RecipeStructureKind, RevisionOrigin,
    RevisionVerification, Servings,
};
use cookbook_domain::recipe::ports::{
    AddRecipeSourceDocumentsError, AddRecipeSourceDocumentsRequest, ConvertRecipeStructureRequest,
    DeleteRecipeSourceDocumentError, RecipeSearchCriteria, RecipeService,
    RestoreRecipeRevisionError, RestoreRecipeRevisionRequest, SearchRecipesQuery,
    UpdateRecipeError, UpdateRecipeScopeError, UpdateRecipeScopeRequest,
};
use cookbook_domain::user::models::UserId;
use cookbook_infra::auth::ports::UserRepository;
use cookbook_vision::detect_source_document_content_type;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        Body,
        content::{Form, multipart::Multipart},
        error::{RouterErrorExt, SeeOther, bad_request, forbidden, not_found, see_other},
        href, page, path_param, query_params,
        response::Response,
        route,
    },
    runtime::{shard, signal},
    view::{View, attributes, view},
};

use crate::{
    app::{AppRecipeService, AppUserRepository, auth},
    components::{
        base::{
            button::button,
            card::{card, card_content, card_header, card_title},
        },
        recipe::{
            category::category_label,
            recipe_form::{LATEST_REVISION_FIELD, RecipeFormMode, read_recipe_form, recipe_form},
            recipe_images::recipe_image_gallery,
            recipe_scope_form::recipe_scope_form,
            recipe_view::recipe_view,
            search_form::{SortChoice, recipe_search_form},
            source_documents::{source_document_editor, source_document_gallery},
        },
    },
    t,
};

pub(crate) const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

const RECIPE_PAGE_SIZE: usize = 20;

/// Query parameters accepted by the recipe list page.
#[query_params(error = bad_request)]
struct RecipeListQuery {
    q: Option<String>,
    sort: Option<String>,
    page: Option<usize>,
    exclude_needs_review: Option<bool>,
    category_aperitif: Option<bool>,
    category_starter: Option<bool>,
    category_main: Option<bool>,
    category_side: Option<bool>,
    category_dessert: Option<bool>,
    category_breakfast: Option<bool>,
    category_snack: Option<bool>,
    category_drink: Option<bool>,
    category_other: Option<bool>,
}

impl RecipeListQuery {
    fn pattern(&self) -> Option<String> {
        self.q
            .as_deref()
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(str::to_string)
    }

    fn category_is_selected(&self, category: RecipeCategory) -> bool {
        let selected = match category {
            RecipeCategory::Aperitif => self.category_aperitif,
            RecipeCategory::Starter => self.category_starter,
            RecipeCategory::Main => self.category_main,
            RecipeCategory::Side => self.category_side,
            RecipeCategory::Dessert => self.category_dessert,
            RecipeCategory::Breakfast => self.category_breakfast,
            RecipeCategory::Snack => self.category_snack,
            RecipeCategory::Drink => self.category_drink,
            RecipeCategory::Other => self.category_other,
        };

        selected.unwrap_or(false)
    }

    fn categories(&self) -> Vec<RecipeCategory> {
        RecipeCategory::ALL
            .into_iter()
            .filter(|category| self.category_is_selected(*category))
            .collect()
    }

    fn exclude_needs_review(&self) -> bool {
        self.exclude_needs_review.unwrap_or(false)
    }

    fn sort_choice(&self) -> Result<SortChoice> {
        SortChoice::parse(self.sort.as_deref())
            .ok_or_else(|| bad_request("unknown recipe sort").into())
    }

    fn page_number(&self) -> usize {
        self.page.unwrap_or(1).max(1)
    }

    fn params(&self) -> Result<SearchParams> {
        Ok(SearchParams {
            pattern: self.pattern(),
            categories: self.categories(),
            exclude_needs_review: self.exclude_needs_review(),
            sort: self.sort_choice()?,
            page: self.page_number(),
        })
    }
}

/// A validated search state, shared by the page and the live-results shard.
struct SearchParams {
    pattern: Option<String>,
    categories: Vec<RecipeCategory>,
    exclude_needs_review: bool,
    sort: SortChoice,
    page: usize,
}

impl SearchParams {
    fn has_filters(&self) -> bool {
        self.pattern.is_some() || !self.categories.is_empty() || self.exclude_needs_review
    }

    fn search_query(&self, page_number: usize) -> SearchRecipesQuery {
        SearchRecipesQuery {
            criteria: RecipeSearchCriteria {
                pattern: self.pattern.clone(),
                categories: self.categories.clone(),
                exclude_needs_review: self.exclude_needs_review,
                sort: self.sort.to_domain(),
            },
            offset: (page_number - 1) * RECIPE_PAGE_SIZE,
            limit: Some(RECIPE_PAGE_SIZE),
        }
    }

    /// Builds a link to another page, preserving every active filter and sort.
    fn page_href(&self, base: &str, target_page: usize) -> String {
        let mut serializer = form_urlencoded::Serializer::new(String::new());

        if let Some(pattern) = &self.pattern {
            serializer.append_pair("q", pattern);
        }

        for category in &self.categories {
            serializer.append_pair(&format!("category_{}", category.as_str()), "true");
        }

        if self.exclude_needs_review {
            serializer.append_pair("exclude_needs_review", "true");
        }

        if self.sort != SortChoice::default() {
            serializer.append_pair("sort", self.sort.as_str());
        }

        if target_page > 1 {
            serializer.append_pair("page", &target_page.to_string());
        }

        let query = serializer.finish();

        if query.is_empty() {
            base.to_string()
        } else {
            format!("{base}?{query}")
        }
    }
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    auth::require_user(cx)?;

    let list_query = query_params::<RecipeListQuery>(cx)?;
    let params = list_query.params()?;
    let query = signal(cx, || params.pattern.clone().unwrap_or_default());
    let category_names = params
        .categories
        .iter()
        .map(|category| category.as_str().to_string())
        .collect::<Vec<_>>();
    let sort = params.sort.as_str().to_string();
    let page_number = params.page;

    Ok(view! {
        cx =>
        <h2 class="mb-6 text-2xl font-semibold">(t!(recipes_list_heading))</h2>

        recipe_search_form(
            query: query.clone(),
            categories: &params.categories,
            exclude_needs_review: params.exclude_needs_review,
            sort: params.sort
        )

        recipe_search_results(
            query: $(query.get()),
            categories: category_names,
            exclude_needs_review: params.exclude_needs_review,
            sort: sort,
            page_number: page_number
        )
    })
}

#[shard]
async fn recipe_search_results(
    cx: &Cx,
    query: String,
    categories: Vec<String>,
    exclude_needs_review: bool,
    sort: String,
    page_number: usize,
) -> Result<impl View> {
    let user = auth::require_user(cx)?;
    let params = SearchParams {
        pattern: Some(query.trim().to_string()).filter(|pattern| !pattern.is_empty()),
        categories: categories
            .iter()
            .filter_map(|category| RecipeCategory::parse(category))
            .collect(),
        exclude_needs_review,
        sort: SortChoice::parse(Some(&sort)).unwrap_or_default(),
        page: page_number.max(1),
    };
    let has_filters = params.has_filters();

    let recipe_service: &AppRecipeService = app_context(cx);
    let mut result = recipe_service
        .search_recipes(params.search_query(params.page), user.id().clone())
        .await?;

    // A bookmarked page may be past the end of the current results, for
    // example after a recipe was deleted. Clamp it and fetch the last page.
    let total_pages = result.total.div_ceil(RECIPE_PAGE_SIZE);
    let page_number = if total_pages == 0 {
        1
    } else {
        params.page.min(total_pages)
    };
    if page_number != params.page {
        result = recipe_service
            .search_recipes(params.search_query(page_number), user.id().clone())
            .await?;
    }

    let total_pages = result.total.div_ceil(RECIPE_PAGE_SIZE);
    let base = href!(page).resolve(cx);
    let previous_page = (page_number > 1).then(|| params.page_href(&base, page_number - 1));
    let next_page = (page_number < total_pages).then(|| params.page_href(&base, page_number + 1));
    let count_label = if result.total == 1 {
        t!(recipe_search_result_count, count = result.total.to_string())
            .resolve(cx)
            .into_owned()
    } else {
        t!(
            recipe_search_result_count_plural,
            count = result.total.to_string()
        )
        .resolve(cx)
        .into_owned()
    };

    Ok(view! {
        if result.total == 0 {
            if has_filters {
                <p class="mt-8 text-sm text-muted-foreground">(t!(recipe_search_empty))</p>
            } else {
                <p class="mt-8 text-sm text-muted-foreground">(t!(recipes_empty))</p>
            }
        } else {
            <p class="mt-8 text-sm text-muted-foreground" role="status">(count_label)</p>
            <ul class="mt-3 flex flex-col gap-2">
                for recipe in &result.recipes {
                    <li
                        class="rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-sm"
                    >
                        <a
                            class="font-medium underline-offset-4 hover:underline"
                            href=(href!(recipe_detail, RecipeId(recipe.id().as_str())))
                        >
                            (recipe.title().as_str())
                        </a>
                        if let Some(category) = recipe.category() {
                            <span
                                class="ml-2 inline-flex items-center rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground"
                            >
                                (category_label(cx, category))
                            </span>
                        }
                        if recipe.needs_review() {
                            <span
                                class="ml-2 inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2 py-0.5 text-xs font-medium text-warning-foreground"
                            >
                                (t!(recipe_needs_review))
                            </span>
                        }
                    </li>
                }
            </ul>
        }

        if total_pages > 1 {
            <nav
                class="mt-6 flex items-center justify-between text-sm"
                aria-label=(t!(recipe_search_pagination))
            >
                if let Some(previous) = previous_page {
                    <a class="underline underline-offset-4" href=(previous)>
                        (t!(recipe_search_previous))
                    </a>
                } else {
                    <span></span>
                }
                <span class="text-muted-foreground">
                    (t!(
                        recipe_search_page,
                        current = page_number.to_string(),
                        total = total_pages.to_string()
                    ))
                </span>
                if let Some(next) = next_page {
                    <a class="underline underline-offset-4" href=(next)>
                        (t!(recipe_search_next))
                    </a>
                } else {
                    <span></span>
                }
            </nav>
        }
    })
}

path_param!(pub recipe_id);
path_param!(revision_id);
path_param!(pub source_document_id);
path_param!(pub image_id);

#[page("./{recipe_id}")]
pub async fn recipe_detail(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let source_documents = recipe_service
        .get_source_documents(recipe.id().clone(), user.id().clone())
        .await?;
    let images = recipe_service
        .get_recipe_images(recipe.id().clone(), user.id().clone())
        .await?;

    let can_edit = recipe.can_edit(user.id());
    let is_author = recipe.author() == user.id();
    let scope = recipe.scope();
    let scope_label = scope_label(cx, scope);
    let author_name = display_name(cx, recipe.author()).await?;
    let total_minutes = recipe.durations().total_duration().as_minutes();

    Ok(view! {
        <div class="flex flex-col gap-8">
            <div>
                <a
                    class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                    href=(href!(page))
                >
                    (t!(recipe_back_to_list))
                </a>
                <div class="mt-3 flex items-start justify-between gap-4">
                    <div class="flex flex-wrap items-center gap-3">
                        <h2 class="text-3xl font-semibold">
                            (recipe.title().as_str())
                        </h2>
                        if recipe.needs_review() {
                            <span
                                class="inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2.5 py-0.5 text-sm font-medium text-warning-foreground"
                            >
                                (t!(recipe_needs_review))
                            </span>
                        }
                        <span
                            class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-sm font-medium text-muted-foreground"
                        >
                            (scope_label)
                        </span>
                        if let Some(category) = recipe.category() {
                            <span
                                class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-sm font-medium text-muted-foreground"
                            >
                                (category_label(cx, category))
                            </span>
                        }
                    </div>
                    <div class="flex flex-col items-end gap-1 text-sm">
                        if can_edit {
                            <a
                                class="underline underline-offset-4"
                                href=(href!(edit_recipe, RecipeId(recipe.id().as_str())))
                            >
                                if recipe.needs_review() {
                                    (t!(recipe_verify))
                                } else {
                                    (t!(recipe_edit))
                                }
                            </a>
                        }
                        <a
                            class="underline underline-offset-4"
                            href=(href!(recipe_history, RecipeId(recipe.id().as_str())))
                        >
                            (t!(recipe_history))
                        </a>
                    </div>
                </div>
                if let Some(description) = recipe.description() {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (description.as_str())
                    </p>
                }
                if recipe.needs_review() {
                    <p
                        role="status"
                        class="mt-4 rounded-lg border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-warning-foreground"
                    >
                        (t!(recipe_needs_review_notice))
                    </p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_author))
                    ": "
                    (author_name)
                </p>
                if total_minutes > 0 {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (t!(recipe_total_duration))
                        ": "
                        (total_minutes)
                        " "
                        (t!(recipe_minutes))
                    </p>
                }
                if let Some(servings) = recipe.content().servings() {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (t!(recipe_servings))
                        ": "
                        (servings_label(cx, servings))
                    </p>
                }
            </div>

            if let Some(body) = recipe.content().simple_body() {
                recipe_image_gallery(
                    recipe_id: recipe.id().as_str(),
                    image_ids: body.images(),
                    images: &images
                )
            }
            recipe_view(
                recipe_id: recipe.id().as_str(),
                content: recipe.content(),
                images: &images
            )
            source_document_gallery(
                recipe_id: recipe.id().as_str(),
                documents: &source_documents
            )
            if is_author {
                <section class="rounded-lg border border-border bg-card p-4">
                    <h3 class="mb-3 text-lg font-semibold">(t!(scope_heading))</h3>
                    recipe_scope_form(
                        recipe_id: recipe.id().as_str().to_string(),
                        scope: scope
                    )
                </section>
            }
        </div>
    })
}

fn servings_label(cx: &Cx, servings: &Servings) -> String {
    match servings {
        Servings::People(count) if *count == 1 => t!(serving_person, count = count.to_string())
            .resolve(cx)
            .into_owned(),
        Servings::People(count) => t!(serving_people, count = count.to_string())
            .resolve(cx)
            .into_owned(),
        Servings::Portions {
            count,
            label: Some(label),
        } => format!("{count} {label}"),
        Servings::Portions { count, label: None } => count.to_string(),
    }
}

#[page("./{recipe_id}/edit")]
pub async fn edit_recipe(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?
        .ok_or_not_found()?;

    if !recipe.can_edit(user.id()) {
        return Err(forbidden().into());
    }

    let source_documents = recipe_service
        .get_source_documents(recipe.id().clone(), user.id().clone())
        .await?;
    let needs_review = recipe.needs_review();
    let mode = RecipeFormMode::Edit {
        recipe_id: RecipeIdValue::new(recipe_id.clone()),
        latest_revision: RecipeRevisionId::new(recipe.revision().id().as_str().to_string()),
        needs_review,
    };
    let form_title = if needs_review {
        t!(form_verify_recipe).resolve(cx).into_owned()
    } else {
        t!(form_edit_recipe).resolve(cx).into_owned()
    };
    let latest_revision = recipe.revision().id().as_str().to_string();
    let (conversion_action, conversion_label, recipe_type_label) =
        if recipe.content().is_multipart() {
            (
                href!(convert_recipe_to_simple, RecipeId(recipe_id.as_str())).resolve(cx),
                t!(form_convert_to_simple).resolve(cx).into_owned(),
                t!(form_recipe_type_multipart).resolve(cx).into_owned(),
            )
        } else {
            (
                href!(convert_recipe_to_multipart, RecipeId(recipe_id.as_str())).resolve(cx),
                t!(form_convert_to_multipart).resolve(cx).into_owned(),
                t!(form_recipe_type_simple).resolve(cx).into_owned(),
            )
        };
    // TODO: update this warning live as ingredient rows change. It currently
    // reflects the loaded recipe and only clears after the edited recipe is saved.
    let has_ingredient_conflicts = recipe.bodies().any(|body| body.has_ingredient_conflicts());

    Ok(view! {
        <div class="flex flex-col gap-6">
            if has_ingredient_conflicts {
                <p
                    role="status"
                    class="rounded-lg border border-warning/40 bg-warning/10 px-4 py-3 text-sm text-warning-foreground"
                >
                    (t!(recipe_ingredient_conflicts_notice))
                </p>
            }
            card(
                card_header(card_title((form_title)))
                card_content(recipe_form(mode: mode))
            )
            card(
                card_header(card_title((t!(form_recipe_type))))
                card_content(
                    <form
                        method="post"
                        action=(conversion_action)
                        class="flex flex-col gap-3"
                    >
                        <input
                            type="hidden"
                            name=(LATEST_REVISION_FIELD)
                            value=(latest_revision)
                        >
                        <p class="text-sm text-muted-foreground">(recipe_type_label)</p>
                        button(attrs: attributes! { type="submit" }, (conversion_label))
                    </form>
                )
            )
            card(
                card_header(card_title((t!(source_documents_heading))))
                card_content(
                    source_document_editor(
                        recipe_id: recipe_id.as_str(),
                        documents: &source_documents
                    )
                )
            )
        </div>
    })
}

#[route(POST "./{recipe_id}/edit")]
pub async fn update_recipe(cx: &Cx, mut multipart: Multipart) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let submission = read_recipe_form(&mut multipart).await?;
    let request = crate::components::recipe::recipe_form::update_recipe_request(
        submission,
        RecipeIdValue::new(recipe_id.clone()),
        user.id().clone(),
    )?;

    match recipe_service.update_recipe(request).await {
        Ok(_) => Ok(see_other(
            href!(recipe_detail, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(UpdateRecipeError::Conflict) => {
            Err(bad_request(t!(recipe_conflict).resolve(cx)).into())
        }
        Err(UpdateRecipeError::NotFound) => Err(not_found().into()),
        Err(UpdateRecipeError::Forbidden) => Err(forbidden().into()),
        Err(UpdateRecipeError::InvalidImageReference) => {
            Err(bad_request(t!(recipe_image_invalid_target).resolve(cx)).into())
        }
        Err(error) => Err(error.into()),
    }
}

#[route(POST "./{recipe_id}/convert-to-multipart")]
pub async fn convert_recipe_to_multipart(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    convert_recipe_type(cx, form, true).await
}

#[route(POST "./{recipe_id}/convert-to-simple")]
pub async fn convert_recipe_to_simple(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    convert_recipe_type(cx, form, false).await
}

async fn convert_recipe_type(
    cx: &Cx,
    form: Vec<(String, String)>,
    multipart: bool,
) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let latest_revision = parse_latest_revision_form(form)?;
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?
        .ok_or_not_found()?;

    if !recipe.can_edit(user.id()) {
        return Err(forbidden().into());
    }

    let to = if multipart {
        RecipeStructureKind::Multipart
    } else {
        RecipeStructureKind::Simple
    };

    match recipe_service
        .convert_recipe_structure(ConvertRecipeStructureRequest {
            id: RecipeIdValue::new(recipe_id.clone()),
            to,
            editor: user.id().clone(),
            latest_revision,
        })
        .await
    {
        Ok(_) => Ok(see_other(
            href!(edit_recipe, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(UpdateRecipeError::Conflict) => {
            Err(bad_request(t!(recipe_conflict).resolve(cx)).into())
        }
        Err(UpdateRecipeError::NotFound) => Err(not_found().into()),
        Err(UpdateRecipeError::Forbidden) => Err(forbidden().into()),
        Err(UpdateRecipeError::InvalidImageReference) => {
            Err(bad_request(t!(recipe_image_invalid_target).resolve(cx)).into())
        }
        Err(error) => Err(error.into()),
    }
}

fn parse_latest_revision_form(pairs: Vec<(String, String)>) -> Result<RecipeRevisionId> {
    let mut latest_revision = None;

    for (key, value) in pairs {
        if key != LATEST_REVISION_FIELD {
            return Err(bad_request(format!("unknown form field `{key}`")).into());
        }

        if latest_revision.replace(value).is_some() {
            return Err(bad_request("duplicate latest revision").into());
        }
    }

    let latest_revision = latest_revision
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| -> topcoat::Error { bad_request("latest revision is required").into() })?;

    Ok(RecipeRevisionId::new(latest_revision))
}

#[route(POST "./{recipe_id}/history/{revision_id}/restore")]
pub async fn restore_recipe_revision(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let revision_id = path_param::<RevisionId>(cx).to_string();
    let latest_revision = parse_latest_revision_form(form)?;
    let recipe_service: &AppRecipeService = app_context(cx);

    match recipe_service
        .restore_recipe_revision(RestoreRecipeRevisionRequest {
            id: RecipeIdValue::new(recipe_id.clone()),
            revision_id: RecipeRevisionId::new(revision_id),
            editor: user.id().clone(),
            latest_revision,
        })
        .await
    {
        Ok(_) => Ok(see_other(
            href!(recipe_detail, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(RestoreRecipeRevisionError::Conflict) => {
            Err(bad_request(t!(recipe_conflict).resolve(cx)).into())
        }
        Err(RestoreRecipeRevisionError::NotFound) => Err(not_found().into()),
        Err(RestoreRecipeRevisionError::Forbidden) => Err(forbidden().into()),
        Err(error) => Err(error.into()),
    }
}

#[route(POST "./{recipe_id}/scope")]
pub async fn update_recipe_scope(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let scope = parse_scope_form(form)?;
    let recipe_service: &AppRecipeService = app_context(cx);

    match recipe_service
        .update_recipe_scope(UpdateRecipeScopeRequest {
            id: RecipeIdValue::new(recipe_id.clone()),
            scope,
            actor: user.id().clone(),
        })
        .await
    {
        Ok(_) => Ok(see_other(
            href!(recipe_detail, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(UpdateRecipeScopeError::NotFound) => Err(not_found().into()),
        Err(UpdateRecipeScopeError::Forbidden) => Err(forbidden().into()),
        Err(error) => Err(error.into()),
    }
}

#[route(GET "./{recipe_id}/source-documents/{source_document_id}")]
pub async fn source_document(cx: &Cx) -> Result<Response> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let document_id = path_param::<SourceDocumentId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let document = recipe_service
        .get_source_document(
            RecipeIdValue::new(recipe_id),
            RecipeSourceDocumentIdValue::new(document_id),
            user.id().clone(),
        )
        .await?
        .ok_or_not_found()?;

    let mut response = Response::builder()
        .header("Content-Type", document.content_type())
        .header("Cache-Control", "private, max-age=3600")
        .header("X-Content-Type-Options", "nosniff");

    if document.content_type() == "application/pdf" {
        response = response.header(
            "Content-Disposition",
            inline_file_name(document.original_name()),
        );
    }

    Ok(response.body(Body::from(document.bytes().to_vec()))?)
}

fn inline_file_name(original_name: Option<&str>) -> String {
    let name = original_name
        .filter(|name| !name.is_empty())
        .unwrap_or("document.pdf");
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_graphic() && character != '"' && character != '\\' {
                character
            } else {
                '_'
            }
        })
        .collect();

    format!("inline; filename=\"{sanitized}\"")
}

#[route(GET "./{recipe_id}/images/{image_id}")]
pub async fn recipe_image(cx: &Cx) -> Result<Response> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let image_id = path_param::<ImageId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let image = recipe_service
        .get_recipe_image(
            RecipeIdValue::new(recipe_id),
            RecipeImageIdValue::new(image_id),
            user.id().clone(),
        )
        .await?
        .ok_or_not_found()?;

    Ok(Response::builder()
        .header("Content-Type", image.content_type())
        .header("Cache-Control", "private, max-age=3600")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(image.bytes().to_vec()))?)
}

#[route(POST "./{recipe_id}/source-documents")]
pub async fn upload_source_documents(cx: &Cx, mut multipart: Multipart) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let mut documents = Vec::new();

    while let Some(part) = multipart.next_field().await? {
        if part.name() != Some("documents") {
            continue;
        }

        let original_name = part.file_name().map(str::to_owned);
        let bytes = part.bytes().await?;
        let Some(content_type) = detect_source_document_content_type(&bytes) else {
            return Err(bad_request(t!(source_documents_unsupported).resolve(cx)).into());
        };

        documents.push(NewRecipeSourceDocument::new(
            content_type.to_string(),
            original_name,
            bytes.to_vec(),
        ));
    }

    if documents.is_empty() {
        return Err(bad_request(t!(source_documents_upload_empty).resolve(cx)).into());
    }

    let recipe_service: &AppRecipeService = app_context(cx);

    match recipe_service
        .add_source_documents(AddRecipeSourceDocumentsRequest {
            recipe_id: RecipeIdValue::new(recipe_id.clone()),
            added_by: user.id().clone(),
            documents,
        })
        .await
    {
        Ok(_) => Ok(see_other(
            href!(edit_recipe, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(AddRecipeSourceDocumentsError::RecipeNotFound) => Err(not_found().into()),
        Err(AddRecipeSourceDocumentsError::Forbidden) => Err(forbidden().into()),
        Err(error) => Err(error.into()),
    }
}

// Not DELETE since its driven by an HTTP form that can only use POST
#[route(POST "./{recipe_id}/source-documents/{source_document_id}/delete")]
pub async fn delete_source_document(cx: &Cx) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let document_id = path_param::<SourceDocumentId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);

    match recipe_service
        .delete_source_document(
            RecipeIdValue::new(recipe_id.clone()),
            RecipeSourceDocumentIdValue::new(document_id),
            user.id().clone(),
        )
        .await
    {
        Ok(_) => Ok(see_other(
            href!(edit_recipe, RecipeId(recipe_id.as_str())).resolve(cx),
        )),
        Err(DeleteRecipeSourceDocumentError::NotFound) => Err(not_found().into()),
        Err(DeleteRecipeSourceDocumentError::Forbidden) => Err(forbidden().into()),
        Err(error) => Err(error.into()),
    }
}

struct HistoryEntry {
    revision: RecipeRevision,
    editor: String,
}

fn revision_origin_label(cx: &Cx, origin: &RevisionOrigin, revisions: &[RecipeRevision]) -> String {
    match origin {
        RevisionOrigin::Created => t!(recipe_history_created).resolve(cx).into_owned(),
        RevisionOrigin::Imported => t!(recipe_history_imported).resolve(cx).into_owned(),
        RevisionOrigin::UserEdit {
            message: Some(message),
        } => message.as_str().to_owned(),
        RevisionOrigin::UserEdit { message: None } => {
            t!(recipe_history_edited).resolve(cx).into_owned()
        }
        RevisionOrigin::StructureConverted { to } => match to {
            RecipeStructureKind::Simple => t!(recipe_history_converted_to_simple)
                .resolve(cx)
                .into_owned(),
            RecipeStructureKind::Multipart => t!(recipe_history_converted_to_multipart)
                .resolve(cx)
                .into_owned(),
        },
        RevisionOrigin::Restored { revision_id } => revisions
            .iter()
            .find(|revision| revision.id() == revision_id)
            .map(|source| {
                t!(
                    recipe_history_restored,
                    timestamp = format_timestamp(source.edited_at())
                )
                .resolve(cx)
                .into_owned()
            })
            .unwrap_or_else(|| {
                t!(recipe_history_restored_fallback)
                    .resolve(cx)
                    .into_owned()
            }),
    }
}

#[page("./{recipe_id}/history")]
pub async fn recipe_history(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let revisions = recipe_service
        .get_recipe_history(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?;

    let mut entries = Vec::with_capacity(revisions.len());

    for revision in revisions.iter().rev() {
        entries.push(HistoryEntry {
            revision: revision.clone(),
            editor: display_name(cx, revision.editor()).await?,
        });
    }

    Ok(view! {
        <div class="flex flex-col gap-6">
            <a
                class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                href=(href!(recipe_detail, RecipeId(recipe_id.as_str())))
            >
                (t!(recipe_back_to_recipe))
            </a>
            <h2 class="text-2xl font-semibold">
                (t!(recipe_history_heading))
                ": "
                (recipe.title().as_str())
            </h2>

            if entries.is_empty() {
                <p class="text-sm text-muted-foreground">(t!(recipe_history_empty))</p>
            } else {
                <ol class="flex flex-col gap-3">
                    for entry in entries {
                        <li
                            class="rounded-lg border border-border bg-card px-4 py-3 text-sm"
                        >
                            <div class="flex items-start justify-between gap-4">
                                <div>
                                    <p class="font-medium">
                                        (revision_origin_label(
                                            cx,
                                            entry.revision.origin(),
                                            &revisions,
                                        ))
                                    </p>
                                    <p class="mt-1">
                                        if entry.revision.verification()
                                            == RevisionVerification::NeedsReview {
                                            <span
                                                class="inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2 py-0.5 text-xs font-medium text-warning-foreground"
                                            >
                                                (t!(recipe_needs_review))
                                            </span>
                                        } else {
                                            <span
                                                class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground"
                                            >
                                                (t!(recipe_verified))
                                            </span>
                                        }
                                    </p>
                                    <p class="mt-1 text-muted-foreground">
                                        (t!(recipe_history_edited_by))
                                        " "
                                        (entry.editor)
                                        " · "
                                        (format_timestamp(entry.revision.edited_at()))
                                    </p>
                                </div>
                                <a
                                    class="underline underline-offset-4"
                                    href=(href!(
                                        recipe_revision,
                                        RecipeId(recipe_id.as_str()),
                                        RevisionId(entry.revision.id().as_str()),
                                    ))
                                >
                                    (t!(recipe_history_view))
                                </a>
                            </div>
                        </li>
                    }
                </ol>
            }
        </div>
    })
}

#[page("./{recipe_id}/history/{revision_id}")]
pub async fn recipe_revision(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;

    let recipe_id = path_param::<RecipeId>(cx).to_string();
    let revision_id = path_param::<RevisionId>(cx).to_string();
    let recipe_service: &AppRecipeService = app_context(cx);
    let recipe = recipe_service
        .get_recipe(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?
        .ok_or_not_found()?;
    let revisions = recipe_service
        .get_recipe_history(RecipeIdValue::new(recipe_id.clone()), user.id().clone())
        .await?;
    let revision = revisions
        .iter()
        .find(|revision| revision.id().as_str() == revision_id.as_str())
        .cloned()
        .ok_or_not_found()?;
    let images = recipe_service
        .get_recipe_images(recipe.id().clone(), user.id().clone())
        .await?;

    let editor = display_name(cx, revision.editor()).await?;
    let author_name = display_name(cx, recipe.author()).await?;
    let total_minutes = revision.content().durations().total_duration().as_minutes();
    let can_edit = recipe.can_edit(user.id());
    let is_current = revision.id() == recipe.revision().id();
    let latest_revision = recipe.revision().id().as_str().to_string();
    let restore_action = href!(
        restore_recipe_revision,
        RecipeId(recipe_id.as_str()),
        RevisionId(revision.id().as_str()),
    )
    .resolve(cx);

    Ok(view! {
        <div class="flex flex-col gap-8">
            <div>
                <a
                    class="text-sm text-muted-foreground underline-offset-4 hover:underline"
                    href=(href!(recipe_history, RecipeId(recipe_id.as_str())))
                >
                    (t!(recipe_back_to_history))
                </a>
                <div class="mt-3 flex flex-wrap items-center gap-3">
                    <h2 class="text-3xl font-semibold">
                        (revision.content().title().as_str())
                    </h2>
                    if revision.verification() == RevisionVerification::NeedsReview {
                        <span
                            class="inline-flex items-center rounded-full border border-warning/40 bg-warning/10 px-2.5 py-0.5 text-sm font-medium text-warning-foreground"
                        >
                            (t!(recipe_needs_review))
                        </span>
                    } else {
                        <span
                            class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-sm font-medium text-muted-foreground"
                        >
                            (t!(recipe_verified))
                        </span>
                    }
                    if let Some(category) = revision.content().category() {
                        <span
                            class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-sm font-medium text-muted-foreground"
                        >
                            (category_label(cx, category))
                        </span>
                    }
                </div>
                if let Some(description) = revision.content().description() {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (description.as_str())
                    </p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_history_edited_by))
                    " "
                    (editor)
                    " · "
                    (format_timestamp(revision.edited_at()))
                </p>
                <p class="mt-2 text-sm">
                    (revision_origin_label(cx, revision.origin(), &revisions))
                </p>
                if total_minutes > 0 {
                    <p class="mt-2 text-sm text-muted-foreground">
                        (t!(recipe_total_duration))
                        ": "
                        (total_minutes)
                        " "
                        (t!(recipe_minutes))
                    </p>
                }
                <p class="mt-2 text-sm text-muted-foreground">
                    (t!(recipe_author))
                    ": "
                    (author_name)
                </p>
                <p class="mt-4 text-sm">
                    <a
                        class="underline underline-offset-4"
                        href=(href!(recipe_detail, RecipeId(recipe_id.as_str())))
                    >
                        (t!(recipe_view_current))
                    </a>
                </p>
                if can_edit && !is_current {
                    <form
                        method="post"
                        action=(restore_action)
                        class="mt-4 flex flex-col gap-2"
                    >
                        <input
                            type="hidden"
                            name=(LATEST_REVISION_FIELD)
                            value=(latest_revision)
                        >
                        <p class="text-sm text-muted-foreground">
                            (t!(recipe_restore_revision_hint))
                        </p>
                        button(
                            attrs: attributes! { type="submit" },
                            (t!(recipe_restore_revision))
                        )
                    </form>
                }
            </div>

            if let Some(body) = revision.content().simple_body() {
                recipe_image_gallery(
                    recipe_id: recipe.id().as_str(),
                    image_ids: body.images(),
                    images: &images
                )
            }
            recipe_view(
                recipe_id: recipe.id().as_str(),
                content: revision.content(),
                images: &images
            )
        </div>
    })
}

fn parse_scope_form(pairs: Vec<(String, String)>) -> Result<RecipeScope> {
    let mut scope = None;

    for (key, value) in pairs {
        if key != "scope" {
            return Err(bad_request(format!("unknown form field `{key}`")).into());
        }

        if scope.replace(value).is_some() {
            return Err(bad_request("duplicate form field `scope`").into());
        }
    }

    let scope =
        scope.ok_or_else(|| -> topcoat::Error { bad_request("scope is required").into() })?;

    RecipeScope::parse(scope.trim()).ok_or_else(|| bad_request("unknown recipe scope").into())
}

fn scope_label(cx: &Cx, scope: RecipeScope) -> String {
    match scope {
        RecipeScope::Private => t!(scope_private).resolve(cx).into_owned(),
        RecipeScope::Viewable => t!(scope_viewable).resolve(cx).into_owned(),
        RecipeScope::Editable => t!(scope_editable).resolve(cx).into_owned(),
    }
}

async fn display_name(cx: &Cx, user_id: &UserId) -> Result<String> {
    let user_repository: &AppUserRepository = app_context(cx);

    Ok(match user_repository.find_by_id(user_id).await? {
        Some(user) => user
            .username()
            .map(|username| username.as_str().to_string())
            .unwrap_or_else(|| user.email().as_str().to_string()),
        None => user_id.as_str().to_string(),
    })
}

fn format_timestamp(edited_at: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = edited_at.into();

    datetime.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn list_query() -> RecipeListQuery {
        RecipeListQuery {
            q: None,
            sort: None,
            page: None,
            exclude_needs_review: None,
            category_aperitif: None,
            category_starter: None,
            category_main: None,
            category_side: None,
            category_dessert: None,
            category_breakfast: None,
            category_snack: None,
            category_drink: None,
            category_other: None,
        }
    }

    #[test]
    fn list_query_collects_the_selected_categories() {
        let mut query = list_query();
        query.category_main = Some(true);
        query.category_dessert = Some(true);
        query.category_side = Some(false);

        assert_eq!(
            query.categories(),
            vec![RecipeCategory::Main, RecipeCategory::Dessert]
        );
    }

    #[test]
    fn list_query_trims_the_pattern_and_ignores_blank_values() {
        let mut query = list_query();
        query.q = Some("  soup  ".to_string());
        assert_eq!(query.pattern().as_deref(), Some("soup"));
        assert!(query.params().unwrap().has_filters());

        query.q = Some("   ".to_string());
        assert_eq!(query.pattern(), None);
        assert!(!query.params().unwrap().has_filters());
    }

    #[test]
    fn list_query_parses_every_sort_and_rejects_unknown_values() {
        for choice in SortChoice::ALL {
            let mut query = list_query();
            query.sort = Some(choice.as_str().to_string());

            assert_eq!(query.sort_choice().unwrap(), choice);
        }

        let mut query = list_query();
        query.sort = Some("random".to_string());

        assert!(query.sort_choice().is_err());
    }

    #[test]
    fn list_query_clamps_the_page_to_at_least_one() {
        let mut query = list_query();

        assert_eq!(query.page_number(), 1);

        query.page = Some(0);
        assert_eq!(query.page_number(), 1);

        query.page = Some(3);
        assert_eq!(query.page_number(), 3);
    }

    #[test]
    fn search_query_maps_pagination_to_an_offset() {
        let mut query = list_query();
        query.page = Some(3);

        let params = query.params().unwrap();
        let search = params.search_query(params.page);

        assert_eq!(search.offset, 2 * RECIPE_PAGE_SIZE);
        assert_eq!(search.limit, Some(RECIPE_PAGE_SIZE));
    }

    #[test]
    fn page_href_preserves_filters_sort_and_page() {
        let mut query = list_query();
        query.q = Some("soupe à l'oignon".to_string());
        query.category_main = Some(true);
        query.category_dessert = Some(true);
        query.exclude_needs_review = Some(true);
        query.sort = Some("title-asc".to_string());

        let href = query.params().unwrap().page_href("/recipes", 2);

        assert!(href.starts_with("/recipes?"));
        assert!(href.contains("q=soupe+%C3%A0+l%27oignon"));
        assert!(href.contains("category_main=true"));
        assert!(href.contains("category_dessert=true"));
        assert!(href.contains("exclude_needs_review=true"));
        assert!(href.contains("sort=title-asc"));
        assert!(href.contains("page=2"));
    }

    #[test]
    fn page_href_omits_defaults_and_the_first_page() {
        let query = list_query();

        assert_eq!(query.params().unwrap().page_href("/recipes", 1), "/recipes");

        let mut sorted = list_query();
        sorted.sort = Some("updated-desc".to_string());

        assert_eq!(
            sorted.params().unwrap().page_href("/recipes", 1),
            "/recipes"
        );
    }

    #[test]
    fn parses_every_scope() {
        assert_eq!(
            parse_scope_form(pairs(&[("scope", "private")])).unwrap(),
            RecipeScope::Private
        );
        assert_eq!(
            parse_scope_form(pairs(&[("scope", "viewable")])).unwrap(),
            RecipeScope::Viewable
        );
        assert_eq!(
            parse_scope_form(pairs(&[("scope", "editable")])).unwrap(),
            RecipeScope::Editable
        );
    }

    #[test]
    fn trims_the_scope_value() {
        assert_eq!(
            parse_scope_form(pairs(&[("scope", "  private  ")])).unwrap(),
            RecipeScope::Private
        );
    }

    #[test]
    fn rejects_missing_unknown_duplicate_and_extra_fields() {
        for entries in [
            vec![],
            vec![("scope", "public")],
            vec![("scope", "private"), ("scope", "editable")],
            vec![("scope", "private"), ("mystery", "x")],
        ] {
            assert!(
                parse_scope_form(pairs(&entries)).is_err(),
                "expected an error for {entries:?}"
            );
        }
    }

    #[test]
    fn labels_a_restored_revision_with_its_source_timestamp() {
        use cookbook_domain::recipe::models::{
            Recipe, RecipeBody, RecipeContent, RecipeDurations, RecipeTitle,
        };

        let cx = Cx::default().with(topcoat::l10n::locale!("en"));
        let recipe = Recipe::new(
            RecipeIdValue::new("soup".to_string()),
            UserId::new("user-1".to_string()),
        );
        let content = |title: &str| {
            RecipeContent::new_simple(
                RecipeTitle::new(title.to_string()),
                None,
                None,
                None,
                RecipeBody::new(
                    RecipeDurations::new(None, None, None),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
            )
            .unwrap()
        };
        let source = RecipeRevision::new(
            RecipeRevisionId::new("revision-1".to_string()),
            recipe.id().clone(),
            content("Old Soup"),
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
            RevisionOrigin::Created,
            RevisionVerification::Verified,
        );
        let restored = RecipeRevision::new(
            RecipeRevisionId::new("revision-2".to_string()),
            recipe.id().clone(),
            content("Current Soup"),
            UserId::new("user-1".to_string()),
            SystemTime::UNIX_EPOCH,
            RevisionOrigin::Restored {
                revision_id: source.id().clone(),
            },
            RevisionVerification::Verified,
        );

        let label = revision_origin_label(&cx, restored.origin(), &[source, restored.clone()]);

        assert!(label.contains("Restored"));
        assert!(label.contains(&format_timestamp(SystemTime::UNIX_EPOCH)));
    }

    #[test]
    fn localizes_people_servings_with_singular_and_plural_forms() {
        let cx = Cx::default().with(topcoat::l10n::locale!("en"));

        // Fluent wraps interpolated values in bidi isolation marks.
        let label = |servings| {
            servings_label(&cx, &servings)
                .replace('\u{2068}', "")
                .replace('\u{2069}', "")
        };

        assert_eq!(label(Servings::people(1)), "1 person");
        assert_eq!(label(Servings::people(4)), "4 people");
    }

    #[test]
    fn displays_portion_servings_with_their_free_form_label() {
        let cx = Cx::default();

        assert_eq!(
            servings_label(&cx, &Servings::portions_with_label(12, "cookies")),
            "12 cookies"
        );
        assert_eq!(servings_label(&cx, &Servings::portions(12)), "12");
    }

    #[test]
    fn builds_a_safe_inline_file_name_for_pdfs() {
        assert_eq!(
            inline_file_name(Some("grandma's recipe.pdf")),
            "inline; filename=\"grandma's_recipe.pdf\""
        );
        assert_eq!(
            inline_file_name(Some("bad\"name\\\r\n.pdf")),
            "inline; filename=\"bad_name___.pdf\""
        );
        assert_eq!(inline_file_name(None), "inline; filename=\"document.pdf\"");
    }
}
