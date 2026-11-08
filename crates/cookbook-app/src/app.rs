use topcoat::{
    Result,
    context::Cx,
    router::{Slot, layout, module_router, page},
    view::{View, view},
};

mod recipes;

pub fn router() -> topcoat::router::Router {
    module_router!().build()
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
    })
}
