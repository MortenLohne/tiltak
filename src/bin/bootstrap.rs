use lambda_runtime::lambda;
use tiltak::aws;

fn main() {
    lambda!(aws::server::handle_aws_event);
}
