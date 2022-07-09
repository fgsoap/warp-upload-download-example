use bytes::BufMut;
use futures::stream::TryStreamExt;
use reqwest::header::HeaderMap;
use std::convert::Infallible;
use warp::{
    http::StatusCode,
    multipart::{FormData, Part},
    Filter, Rejection, Reply,
};

#[tokio::main]
async fn main() {
    let headers = warp::header::headers_cloned();
    let upload_route = warp::path("upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(5000000000))
        .and(headers)
        .and_then(upload);

    let router = upload_route.recover(handle_rejection);
    println!("Server started at localhost:8080");
    warp::serve(router).run(([0, 0, 0, 0], 8080)).await;
}

async fn upload(form: FormData, headers: HeaderMap) -> Result<impl Reply, Rejection> {
    let parts: Vec<Part> = form.try_collect().await.map_err(|e| {
        eprintln!("form error: {}", e);
        warp::reject::reject()
    })?;

    let x_ms_blob_account = match headers.get("x-ms-blob-account") {
        Some(content_type) => content_type.to_str().unwrap(),
        None => "url",
    };

    let x_ms_blob_sv = match headers.get("x-ms-blob-sv") {
        Some(content_type) => content_type.to_str().unwrap(),
        None => "sv",
    };

    let x_ms_blob_container = match headers.get("x-ms-blob-container") {
        Some(content_type) => content_type.to_str().unwrap(),
        None => "container",
    };

    for p in parts {
        let url = format!(
            "https://{}.blob.core.windows.net/{}/{}{}",
            x_ms_blob_account,
            x_ms_blob_container,
            p.filename().unwrap(),
            x_ms_blob_sv
        );

        let value = p
            .stream()
            .try_fold(Vec::new(), |mut vec, data| {
                vec.put(data);
                async move { Ok(vec) }
            })
            .await
            .map_err(|e| {
                eprintln!("reading file error: {}", e);
                warp::reject::reject()
            })?;

        let part = reqwest::multipart::Part::bytes(value);
        let file = reqwest::multipart::Form::new().part("part_bytes", part);

        let mut headers = HeaderMap::new();
        headers.insert("x-ms-blob-type", "BlockBlob".parse().unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        let _res = client.put(url).multipart(file).send().await.unwrap();
        println!("{:?}", _res);
    }

    Ok("success")
}

async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        (StatusCode::BAD_REQUEST, "Payload too large".to_string())
    } else {
        eprintln!("unhandled error: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    Ok(warp::reply::with_status(message, code))
}
