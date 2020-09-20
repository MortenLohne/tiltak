use lambda_runtime::lambda;
use taik::aws;

fn main() {
    lambda!(aws::handle_aws_event);
}
