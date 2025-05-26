# infrastructure/cdk

Contains the AWS CDK (Cloud Development Kit) application for deploying the OpenLifter WebSocket server infrastructure.

- `cdk_app.py`: Main entrypoint for the CDK app, defining AWS resources and deployment logic.

## Main AWS Resources Provisioned
- VPC (Virtual Private Cloud)
- ECS Cluster and Services (for running the server)
- Application Load Balancer
- Security Groups
- IAM Roles and Policies
- S3 Buckets (if used for storage)
- CloudWatch Logs

## Usage
Install dependencies and deploy with:
```bash
pip install -r requirements.txt
cdk deploy
``` 