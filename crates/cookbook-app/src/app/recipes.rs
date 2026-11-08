use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::{View, view},
};

#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let _ = cx;

    Ok(view! {
        <h2>"Recipe list"</h2>
        <p>"No recipes yet."</p>
    })
}
