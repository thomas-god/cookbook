use cookbook_domain::recipe::service::RecipeServiceImpl;
use cookbook_infra::recipe::InMemoryRecipeRepository;
use topcoat::{
    Result,
    context::Cx,
    router::{Slot, layout, module_router, page},
    view::{View, view},
};

mod recipes;

pub(crate) type AppRecipeService = RecipeServiceImpl<InMemoryRecipeRepository>;

pub fn router() -> topcoat::router::Router {
    let recipe_service = AppRecipeService::new(InMemoryRecipeRepository::new());

    module_router!().app_context(recipe_service).build()
}

#[layout]
async fn root_layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let _ = cx;

    Ok(view! {
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8">
                <title>"Cookbook"</title>
                topcoat::dev::script()
            </head>
            <body>
                <header>
                    <h1>"Cookbook"</h1>
                </header>
                <main>
                    (slot)
                </main>
            </body>
        </html>
    })
}

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let _ = cx;

    Ok(view! {
        <h2>"Recipes"</h2>
        <p>"Welcome to the family cookbook."</p>
        <form method="post" action="/recipes">
            <label>
                "Title"
                <input type="text" name="title" required="">
            </label>
            <button type="submit">"Add recipe"</button>
        </form>
    })
}
