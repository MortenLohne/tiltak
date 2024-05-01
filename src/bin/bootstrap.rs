use lambda_runtime::service_fn;
use tiltak::aws;

type Error = Box<dyn std::error::Error + Sync + Send>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(aws::server::handle_aws_event)).await?;
    Ok(())
}
