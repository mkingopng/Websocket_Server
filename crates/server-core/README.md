# server-core

This crate contains the **core business logic** for the OpenLifter backend. It provides the main services, state management, and orchestration logic used by the application entrypoint (`server-app`).

## Purpose
- Encapsulate reusable backend logic and services
- Provide APIs for meet management, authentication, and message routing
- Serve as the main dependency for the application binary

## Key Modules
- `src/`: Core logic and services
- `config/`: Default configuration files
- `data/`: Sample or persistent data for development/testing

## Usage
This crate is not intended to be run directly. It is used as a library by `server-app` and other binaries.
