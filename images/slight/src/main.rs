use anyhow::Result;

use http::*;
use slight_http_handler_macro::register_handler;

wit_bindgen_rust::import!("http.wit");
wit_error_rs::impl_error!(Error);

fn main() -> Result<()> {
    let router = Router::new()?;
    let router_with_route = router
        .get("/hello", "handle_hello")?
        .get("/foo", "handle_foo")?
        .put("/bar", "handle_bar")?
        .post("/upload", "upload")?
        .delete("/delete-file", "delete_file_handler")?;

    println!("guest starting server");
    let _ = Server::serve("0.0.0.0:80", &router_with_route)?;
    // server.stop().unwrap();
    println!("guest moving on");

    Ok(())
}

#[register_handler]
fn handle_hello(req: Request) -> Result<Response, Error> {
    Ok(Response {
        headers: Some(req.headers),
        body: Some("hello".as_bytes().to_vec()),
        status: 200,
    })
}

#[register_handler]
fn handle_foo(request: Request) -> Result<Response, Error> {
    Ok(Response {
        headers: Some(request.headers),
        body: request.body,
        status: 500,
    })
}

#[register_handler]
fn handle_bar(request: Request) -> Result<Response, Error> {
    assert_eq!(request.method, Method::Put);
    Ok(Response {
        headers: Some(request.headers),
        body: request.body,
        status: 200,
    })
}

#[register_handler]
fn delete_file_handler(request: Request) -> Result<Response, Error> {
    assert_eq!(request.method, Method::Delete);
    Ok(Response {
        headers: Some(request.headers),
        body: request.body,
        status: 200,
    })
}


#[register_handler]
fn upload(request: Request) -> Result<Response, Error> {
    assert_eq!(request.method, Method::Post);
    Ok(Response {
        headers: Some(request.headers),
        body: request.body,
        status: 200,
    })
}
