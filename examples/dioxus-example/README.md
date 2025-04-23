# Clerk Dioxus Example

This example demonstrates using Clerk for authentication in a Dioxus application.

## Getting Started

To run this example:

1. Set your Clerk publishable key as an environment variable:
   ```bash
   export CLERK_PUBLISHABLE_KEY="your_publishable_key"
   ```

2. Run the example with the standard Dioxus tooling.

## Implementation Notes

This example includes a custom `use_clerk` hook in `src/use_clerk.rs` that provides:

- ClerkContext for sharing authentication state
- ClerkProvider component for initialization
- Helper hooks for auth state access
- Sign-in and sign-out functionality

The UI components demonstrate reactive updates based on authentication state.

> **Note**: The implementation is a work in progress. The RSX syntax in Dioxus is version-specific and may require adjustments based on your exact Dioxus version. The current code provides a strong foundation but may need fixes to match Dioxus's latest API.

## Project Structure

```
project/
├─ assets/ # Any assets that are used by the app should be placed here
├─ src/
│  ├─ main.rs # main.rs is the entry point to your application 
│  ├─ use_clerk.rs # Custom hooks and components for Clerk integration
├─ Cargo.toml # The Cargo.toml file defines the dependencies and feature flags for your project
```

### Tailwind
1. Install npm: https://docs.npmjs.com/downloading-and-installing-node-js-and-npm
2. Install the Tailwind CSS CLI: https://tailwindcss.com/docs/installation
3. Run the following command in the root of the project to start the Tailwind CSS compiler:

```bash
npx tailwindcss -i ./input.css -o ./assets/tailwind.css --watch
```

### Serving Your App

Run the following command in the root of your project to start developing with the default platform:

```bash
dx serve --platform web
```

To run for a different platform, use the `--platform platform` flag. E.g.
```bash
dx serve --platform desktop
```

