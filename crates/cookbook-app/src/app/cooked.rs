use chrono::NaiveDate;
use cookbook_domain::recipe::models::RecipeCookedEventId;
use cookbook_domain::recipe::ports::{
    AddRecipeCookedEventError, AddRecipeCookedEventRequest, DeleteRecipeCookedEventError,
    ListUserCookedEventsQuery, RecipeService, UpdateRecipeCookedEventError,
    UpdateRecipeCookedEventRequest,
};
use topcoat::icon::icon;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{
        content::Form,
        error::{RouterErrorExt, SeeOther, bad_request, not_found, see_other},
        href, page, path_param, query_params, route,
    },
    view::{View, view},
};

use crate::components::base::icons::BACK_ARROW;
use crate::{
    app::{AppRecipeService, auth, recipes::recipe_id},
    components::recipe::cooked_event::{cooked_event_log, recipe_cooked_events},
    t,
};

const COOKED_LOG_PAGE_SIZE: usize = 20;

path_param!(pub cooked_event_id);

#[query_params(error = bad_request)]
struct CookedLogQuery {
    page: Option<usize>,
}

impl CookedLogQuery {
    fn page_number(&self) -> usize {
        self.page.unwrap_or(1).max(1)
    }
}

fn page_href(base: &str, page_number: usize) -> String {
    if page_number > 1 {
        format!("{base}?page={page_number}")
    } else {
        base.to_string()
    }
}

#[page("/cooked")]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;
    let query = query_params::<CookedLogQuery>(cx)?;
    let requested_page = query.page_number();
    let recipe_service: &AppRecipeService = context(cx);

    let mut result = recipe_service
        .list_user_cooked_events(ListUserCookedEventsQuery {
            user_id: user.id().clone(),
            offset: (requested_page - 1) * COOKED_LOG_PAGE_SIZE,
            limit: COOKED_LOG_PAGE_SIZE,
        })
        .await?;

    let total_pages = result.total.div_ceil(COOKED_LOG_PAGE_SIZE);
    let current_page = if total_pages == 0 {
        1
    } else {
        requested_page.min(total_pages)
    };
    if current_page != requested_page {
        result = recipe_service
            .list_user_cooked_events(ListUserCookedEventsQuery {
                user_id: user.id().clone(),
                offset: (current_page - 1) * COOKED_LOG_PAGE_SIZE,
                limit: COOKED_LOG_PAGE_SIZE,
            })
            .await?;
    }

    let total_pages = result.total.div_ceil(COOKED_LOG_PAGE_SIZE);
    let base = href!(page).resolve(cx);
    let previous_page = (current_page > 1).then(|| page_href(&base, current_page - 1));
    let next_page = (current_page < total_pages).then(|| page_href(&base, current_page + 1));
    let next = page_href(&base, current_page);

    Ok(view! {
        <div class="flex flex-col gap-6">
            <div>
                <h2 class="text-2xl font-semibold">(t!(cooked_event_log_heading))</h2>
                <p class="mt-1 text-sm text-muted-foreground">
                    (t!(cooked_event_log_hint))
                </p>
            </div>

            cooked_event_log(entries: result.events.as_slice(), next: next.as_str())

            if total_pages > 1 {
                <nav
                    class="flex items-center justify-between text-sm"
                    aria-label=(t!(cooked_event_log_pagination))
                >
                    if let Some(previous) = previous_page {
                        <a class="underline underline-offset-4" href=(previous)>
                            (t!(cooked_event_previous))
                        </a>
                    } else {
                        <span></span>
                    }
                    <span class="text-muted-foreground">
                        (t!(
                            cooked_event_page,
                            current: current_page.to_string(),
                            total: total_pages.to_string(),
                        ))
                    </span>
                    if let Some(next) = next_page {
                        <a class="underline underline-offset-4" href=(next)>
                            (t!(cooked_event_next))
                        </a>
                    } else {
                        <span></span>
                    }
                </nav>
            }
        </div>
    })
}

#[page("/recipe/{recipe_id}/cooked")]
pub async fn recipe_cook_journal(cx: &Cx) -> Result<impl View> {
    let user = auth::require_user(cx)?;
    let recipe_id = recipe_id(cx).to_string();
    let recipe_service: &AppRecipeService = context(cx);
    let recipe = recipe_service
        .get_recipe(
            cookbook_domain::recipe::models::RecipeId::new(recipe_id),
            user.id().clone(),
        )
        .await?
        .ok_or_not_found()?;
    let events = recipe_service
        .list_recipe_cooked_events(recipe.id().clone(), user.id().clone())
        .await?;
    let back_href = href!(
        crate::app::recipe::view::recipe_detail,
        recipe_id: recipe.id().as_str()
    )
    .resolve(cx);

    Ok(view! {
        <div class="flex flex-col gap-6">
            <a
                class="text-sm text-muted-foreground underline-offset-4 hover:underline italic opacity-80 flex items-center gap-1.5"
                href=(back_href)
            >
                icon(data: BACK_ARROW)
                (t!(recipe_back_to_recipe))
            </a>
            <div>
                <h2 class="text-2xl font-semibold">
                    (t!(cooked_event_heading))
                    ": "
                    (recipe.title().as_str())
                </h2>
                <p class="mt-1 text-sm text-muted-foreground">
                    (t!(cooked_event_hint))
                </p>
            </div>
            recipe_cooked_events(recipe_id: recipe.id().as_str(), events: &events)
        </div>
    })
}

#[route(POST "/recipe/{recipe_id}/cooked")]
pub async fn add_recipe_cooked_event(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let recipe_id = recipe_id(cx).to_string();
    let form = CookedEventForm::parse(form).ok_or_else(|| -> topcoat::Error {
        bad_request(t!(cooked_event_invalid_form).resolve(cx)).into()
    })?;
    let recipe_service: &AppRecipeService = context(cx);

    match recipe_service
        .add_recipe_cooked_event(AddRecipeCookedEventRequest {
            recipe_id: cookbook_domain::recipe::models::RecipeId::new(recipe_id.clone()),
            user_id: user.id().clone(),
            cooked_on: form.cooked_on,
            note: form.note,
        })
        .await
    {
        Ok(_) => Ok(see_other(
            href!(recipe_cook_journal, recipe_id: recipe_id.as_str()).resolve(cx),
        )),
        Err(AddRecipeCookedEventError::NotFound) => Err(not_found().into()),
        Err(AddRecipeCookedEventError::InvalidNote) => {
            Err(bad_request(t!(cooked_event_note_too_long).resolve(cx)).into())
        }
        Err(error) => Err(error.into()),
    }
}

#[route(POST "/cooked/{cooked_event_id}/update")]
pub async fn update_cooked_event(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let event_id = cooked_event_id(cx).to_string();
    let form = CookedEventForm::parse(form).ok_or_else(|| -> topcoat::Error {
        bad_request(t!(cooked_event_invalid_form).resolve(cx)).into()
    })?;
    let next = redirect_target(form.next, "/cooked".to_string());
    let recipe_service: &AppRecipeService = context(cx);

    match recipe_service
        .update_recipe_cooked_event(UpdateRecipeCookedEventRequest {
            id: RecipeCookedEventId::new(event_id),
            user_id: user.id().clone(),
            cooked_on: form.cooked_on,
            note: form.note,
        })
        .await
    {
        Ok(()) => Ok(see_other(next)),
        Err(UpdateRecipeCookedEventError::NotFound) => Err(not_found().into()),
        Err(UpdateRecipeCookedEventError::InvalidNote) => {
            Err(bad_request(t!(cooked_event_note_too_long).resolve(cx)).into())
        }
        Err(error) => Err(error.into()),
    }
}

#[route(POST "/cooked/{cooked_event_id}/delete")]
pub async fn delete_cooked_event(
    cx: &Cx,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<SeeOther> {
    let user = auth::require_user(cx)?;
    let event_id = cooked_event_id(cx).to_string();
    let next = redirect_target(
        form.into_iter()
            .find(|(key, _)| key == "next")
            .map(|(_, value)| value),
        "/cooked".to_string(),
    );
    let recipe_service: &AppRecipeService = context(cx);

    match recipe_service
        .delete_recipe_cooked_event(RecipeCookedEventId::new(event_id), user.id().clone())
        .await
    {
        Ok(()) => Ok(see_other(next)),
        Err(DeleteRecipeCookedEventError::NotFound) => Err(not_found().into()),
        Err(error) => Err(error.into()),
    }
}

/// Form values for creating or updating a cooked event.
struct CookedEventForm {
    cooked_on: NaiveDate,
    note: String,
    next: Option<String>,
}

impl CookedEventForm {
    fn parse(pairs: Vec<(String, String)>) -> Option<Self> {
        let mut cooked_on = None;
        let mut note = None;
        let mut next = None;

        for (key, value) in pairs {
            match key.as_str() {
                "cooked_on" => {
                    if cooked_on.replace(value).is_some() {
                        return None;
                    }
                }
                "note" => {
                    if note.replace(value).is_some() {
                        return None;
                    }
                }
                "next" => {
                    if next.replace(value).is_some() {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        let cooked_on = cooked_on?;
        let cooked_on = NaiveDate::parse_from_str(cooked_on.trim(), "%Y-%m-%d").ok()?;

        Some(Self {
            cooked_on,
            note: note.unwrap_or_default(),
            next: next.filter(|value| !value.is_empty()),
        })
    }
}

fn redirect_target(next: Option<String>, fallback: String) -> String {
    next.filter(|target| {
        target.starts_with('/')
            && !target.starts_with("//")
            && !target.contains('\\')
            && target.chars().all(|character| !character.is_control())
    })
    .unwrap_or(fallback)
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

    #[test]
    fn parses_a_cooked_event_form() {
        let form = CookedEventForm::parse(pairs(&[
            ("cooked_on", "2026-11-20"),
            ("note", "  Turned out great.  "),
            ("next", "/cooked?page=2"),
        ]))
        .unwrap();

        assert_eq!(form.cooked_on.to_string(), "2026-11-20");
        assert_eq!(form.note, "  Turned out great.  ");
        assert_eq!(form.next.as_deref(), Some("/cooked?page=2"));
    }

    #[test]
    fn accepts_an_omitted_note() {
        let form = CookedEventForm::parse(pairs(&[("cooked_on", "2026-11-20")])).unwrap();

        assert!(form.note.is_empty());
        assert_eq!(form.cooked_on.to_string(), "2026-11-20");
    }

    #[test]
    fn rejects_invalid_dates_unknown_fields_and_duplicates() {
        assert!(CookedEventForm::parse(pairs(&[("cooked_on", "not-a-date")])).is_none());
        assert!(
            CookedEventForm::parse(pairs(&[("cooked_on", "2026-11-20"), ("unknown", "value"),]))
                .is_none()
        );
        assert!(
            CookedEventForm::parse(pairs(&[
                ("cooked_on", "2026-11-20"),
                ("cooked_on", "2026-11-21"),
            ]))
            .is_none()
        );
    }

    #[test]
    fn redirect_target_only_allows_internal_paths() {
        assert_eq!(
            redirect_target(Some("/cooked".to_string()), "/fallback".to_string()),
            "/cooked"
        );
        assert_eq!(
            redirect_target(
                Some("https://example.com".to_string()),
                "/fallback".to_string()
            ),
            "/fallback"
        );
        assert_eq!(
            redirect_target(Some("//example.com".to_string()), "/fallback".to_string()),
            "/fallback"
        );
    }
}
