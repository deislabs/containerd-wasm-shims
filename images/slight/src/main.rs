use anyhow::Result;

use http_server::*;
use keyvalue::*;
use slight_http_handler_macro::register_handler;

wit_bindgen_rust::import!("http-server.wit");
wit_bindgen_rust::import!("keyvalue.wit");
wit_error_rs::impl_error!(http_server::HttpRouterError);
wit_error_rs::impl_error!(keyvalue::KeyvalueError);

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
    println!("guest moving on");

    Ok(())
}

#[register_handler]
fn handle_hello(req: Request) -> Result<Response, HttpError> {
    Ok(Response {
        headers: Some(req.headers),
        body: Some("hello".as_bytes().to_vec()),
        status: 200,
    })
}

#[register_handler]
fn handle_foo(request: Request) -> Result<Response, HttpError> {
    let keyvalue =
        Keyvalue::open("my-container").map_err(|e| HttpError::UnexpectedError(e.to_string()))?;
    match keyvalue.get("key") {
        Err(KeyvalueError::KeyNotFound(_)) => Ok(Response {
            headers: Some(request.headers),
            body: Some("Key not found".as_bytes().to_vec()),
            status: 404,
        }),
        Ok(value) => Ok(Response {
            headers: Some(request.headers),
            body: Some(value),
            status: 200,
        }),
        Err(e) => Err(HttpError::UnexpectedError(e.to_string())),
    }
}

#[register_handler]
fn handle_bar(request: Request) -> Result<Response, HttpError> {
    assert_eq!(request.method, Method::Put);
    if let Some(body) = request.body {
        let keyvalue = Keyvalue::open("my-container")
            .map_err(|e| HttpError::UnexpectedError(e.to_string()))?;
        keyvalue
            .set("key", &body)
            .map_err(|e| HttpError::UnexpectedError(e.to_string()))?;
    }
    Ok(Response {
        headers: Some(request.headers),
        body: None,
        status: 204,
    })
}

#[register_handler]
fn delete_file_handler(request: Request) -> Result<Response, HttpError> {
    assert_eq!(request.method, Method::Delete);
    Ok(Response {
        headers: Some(request.headers),
        body: request.body,
        status: 200,
    })
}

#[register_handler]
fn upload(request: Request) -> Result<Response, HttpError> {
    assert_eq!(request.method, Method::Post);
    Ok(Response {
        headers: Some(request.headers),
        body: request.body,
        status: 200,
    })
}
