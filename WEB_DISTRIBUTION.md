# Web App Distribution with Vortex Server

The Vortex Server is configured to serve static files from the `static/` directory. To ship the web app with the server, follow these steps:

1.  **Build the Web App**:
    Navigate to `vortex-client` and run:
    ```bash
    npm run build
    ```
    This creates a `dist/` directory with the compiled HTML, CSS, and JS.

2.  **Deploy to Server**:
    Copy the contents of `vortex-client/dist/` into `vortex-server/static/`.

3.  **Run Server**:
    When `vortex-server` starts, it will serve `index.html` at the root URL (`http://localhost:3000/`).

## Automation
We will add a build script to automate this process.
