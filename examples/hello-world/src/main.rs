use topcoat::{
    Result,
    router::{module_router, page},
    story::story,
    view::{View, component, view},
};

#[tokio::main]
async fn main() {
    // `module_router!` derives page, layout, and route paths from Rust modules.
    topcoat::start(module_router!().build()).await.unwrap();
}

#[page]
async fn home() -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>"Hello world"</title>

                // Reloads the browser when the dev server rebuilds the app.
                topcoat::dev::script()
            </head>
            <body>hello(name: "World")</body>
        </html>
    })
}

#[component]
async fn hello(name: &str) -> Result<impl View> {
    Ok(view! {
        <h1>
            "Hello, "
            (name)
            "!"
        </h1>
    })
}

/// Says hello to nobody in particular.
#[story(group = "Smoke", component = "Hello", name = "Default")]
#[component]
async fn hello_default() -> Result<impl View> {
    Ok(view! { <p>"smoke story"</p> })
}
