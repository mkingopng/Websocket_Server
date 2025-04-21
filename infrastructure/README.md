# OpenLifter WebSocket Server Infrastructure

This directory contains infrastructure-related code for the OpenLifter WebSocket server.

## Directory Structure

- `cdk/` - Python CDK app for AWS infrastructure
- `Dockerfile` - Docker configuration for the server
- `README.md` - This file

## Deployment

The server can be deployed using the CDK app in the `cdk/` directory.

```bash
cd cdk
pip install -r requirements.txt
cdk deploy
```

## Docker

The server can be built and run using Docker:

```bash
docker build -t openlifter-ws-server .
docker run -p 3000:3000 openlifter-ws-server
``` 