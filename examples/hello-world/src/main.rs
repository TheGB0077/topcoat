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

/// Says hello to whoever is passed, defaulting to the fixture.
/// The `name` control writes the `?name=` query key, which the macro
/// wires straight into the prop.
#[story(
    group = "Grouping",
    component = "Hello World",
    name = "Test Component 2",
    params = [text("name")],
    fixtures = [name = "World"]
)]
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

/// A Test for the story command.
#[story(group = "Grouping", component = "Hello", name = "Test Component")]
#[component]
async fn hello_default() -> Result<impl View> {
    Ok(view! { <p>"Test story!"</p> })
}
