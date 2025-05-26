#!/usr/bin/env python3

from aws_cdk import (
    aws_ec2 as ec2,
    aws_ecs as ecs,
    aws_elasticloadbalancingv2 as elbv2,
    aws_iam as iam,
    aws_logs as logs,
    core,
)

class OpenLifterStack(core.Stack):
    def __init__(self, scope: core.Construct, id: str, **kwargs) -> None:
        super().__init__(scope, id, **kwargs)

        # Create a VPC
        vpc = ec2.Vpc(self, "OpenLifterVPC", max_azs=2)

        # Create an ECS cluster
        cluster = ecs.Cluster(self, "OpenLifterCluster", vpc=vpc)

        # Create a task definition
        task_definition = ecs.FargateTaskDefinition(self, "OpenLifterTask",
            memory_limit_mib=512,
            cpu=256,
        )

        # Add a container to the task definition
        container = task_definition.add_container("OpenLifterContainer",
            image=ecs.ContainerImage.from_asset("."),
            logging=ecs.LogDrivers.aws_logs(
                stream_prefix="OpenLifter",
                log_retention=logs.RetentionDays.ONE_WEEK,
            ),
        )

        # Add port mappings
        container.add_port_mappings(ecs.PortMapping(container_port=3000))

        # Create a security group for the load balancer
        lb_security_group = ec2.SecurityGroup(self, "LBSecurityGroup",
            vpc=vpc,
            description="Security group for the load balancer",
        )

        # Allow inbound traffic on port 80
        lb_security_group.add_ingress_rule(
            ec2.Peer.any_ipv4(),
            ec2.Port.tcp(80),
            "Allow HTTP traffic",
        )

        # Create a load balancer
        lb = elbv2.ApplicationLoadBalancer(self, "OpenLifterLB",
            vpc=vpc,
            internet_facing=True,
            security_group=lb_security_group,
        )

        # Add a listener to the load balancer
        listener = lb.add_listener("Listener",
            port=80,
        )

        # Create a target group
        target_group = elbv2.ApplicationTargetGroup(self, "OpenLifterTargetGroup",
            vpc=vpc,
            port=3000,
            protocol=elbv2.ApplicationProtocol.HTTP,
            target_type=elbv2.TargetType.IP,
        )

        # Add the target group to the listener
        listener.add_target_groups("OpenLifterTargetGroup", target_groups=[target_group])

        # Create a service
        service = ecs.FargateService(self, "OpenLifterService",
            cluster=cluster,
            task_definition=task_definition,
            desired_count=1,
            assign_public_ip=True,
            security_groups=[lb_security_group],
        )

        # Add the service to the target group
        service.attach_to_application_target_group(target_group)

        # Output the load balancer DNS name
        core.CfnOutput(self, "LoadBalancerDNS", value=lb.load_balancer_dns_name)

app = core.App()
OpenLifterStack(app, "OpenLifterStack")
app.synth()
