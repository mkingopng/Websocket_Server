from aws_cdk import (
    Stack,
    aws_ec2 as ec2,
    aws_ecs as ecs,
    aws_ecs_patterns as ecs_patterns,
    aws_certificatemanager as acm,
    aws_route53 as route53,
    aws_route53_targets as targets,
)
from constructs import Construct

class WSBackendStack(Stack):
    def __init__(self, scope: Construct, construct_id: str, **kwargs):
        super().__init__(scope, construct_id, **kwargs)

        # Lookup your hosted zone
        zone = route53.HostedZone.from_lookup(
            self, "HostedZone", domain_name="apl-lights.com"
        )

        # Request a TLS cert for subdomain
        cert = acm.Certificate(
            self, "BackendCert",
            domain_name="server-app.apl-lights.com",
            validation=acm.CertificateValidation.from_dns(zone)
        )

        # Create VPC and ECS cluster
        vpc = ec2.Vpc(self, "WSVpc", max_azs=2)
        cluster = ecs.Cluster(self, "WSCluster", vpc=vpc)

        # Deploy Fargate Service + ALB
        fargate_service = ecs_patterns.ApplicationLoadBalancedFargateService(
            self, "WSBackendService",
            cluster=cluster,
            cpu=256,
            desired_count=1,
            memory_limit_mib=512,
            public_load_balancer=True,
            domain_name="server-app.apl-lights.com",
            domain_zone=zone,
            certificate=cert,
            listener_port=443,
            task_image_options=ecs_patterns.ApplicationLoadBalancedTaskImageOptions(
                image=ecs.ContainerImage.from_registry(
                    "123456789012.dkr.ecr.ap-southeast-2.amazonaws.com/ws-server-app"
                ),
                container_port=9001,
            )
        )

        # Optional: Health check path
        fargate_service.target_group.configure_health_check(path="/health")

        # Optional: DNS record
        route53.ARecord(
            self, "BackendDNS",
            zone=zone,
            target=route53.RecordTarget.from_alias(targets.LoadBalancerTarget(fargate_service.load_balancer)),
            record_name="server-app"
        )
