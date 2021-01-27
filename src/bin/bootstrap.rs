use lambda_runtime::lambda;
use taik::aws;

fn main() {
    lambda!(aws::server::handle_aws_event);
}
