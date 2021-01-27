use crate::aws::{Event, Output};
use bytes::Bytes;
use log::{debug, error, warn};
use rusoto_core::Region;
use rusoto_lambda::{InvocationRequest, Lambda, LambdaClient};
use std::io;

/// Clientside function for receiving moves from AWS
pub fn best_move_aws(aws_function_name: &str, payload: &Event) -> io::Result<Output> {
    let client = LambdaClient::new(Region::UsEast2);

    let request = InvocationRequest {
        client_context: None,
        function_name: aws_function_name.to_string(),
        invocation_type: Some("RequestResponse".to_string()),
        log_type: None,
        payload: Some(Bytes::copy_from_slice(
            &serde_json::to_vec(payload).unwrap(),
        )),
        qualifier: None,
    };

    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async { client.invoke(request).await });
    match result {
        Ok(response) => {
            if let Some(status_code) = response.status_code {
                if status_code / 100 == 2 {
                    debug!("Got HTTP response {} from aws", status_code);
                } else {
                    error!("Got HTTP response {} from aws", status_code);
                }
            } else {
                warn!("AWS response contained no status code");
            }
            if let Some(payload) = response.payload {
                Ok(serde_json::from_str(
                    std::str::from_utf8(&payload)
                        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?,
                )?)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "AWS response contained no payload",
                ))
            }
        }
        Err(err) => Err(io::Error::new(io::ErrorKind::Other, err)),
    }
}
