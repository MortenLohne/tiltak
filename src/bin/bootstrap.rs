use lambda_runtime::handler_fn;
use tiltak::aws;

type Error = Box<dyn std::error::Error + Sync + Send>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(handler_fn(aws::server::handle_aws_event)).await?;
    Ok(())
}
