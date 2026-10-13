import {
  isRouteErrorResponse,
  Link,
  Links,
  Meta,
  Outlet,
  Scripts,
  ScrollRestoration,
} from "react-router";
import type { Route } from "./+types/root";
import "./index.css";
import { StrictMode } from "react";
import satoshiFont from "./assets/fonts/Satoshi-Regular.woff2";
import satoshiFontBold from "./assets/fonts/Satoshi-Bold.woff2";
import satoshiFontMedium from "./assets/fonts/Satoshi-Medium.woff2";
import satoshiFontBlack from "./assets/fonts/Satoshi-Black.woff2";
import jetbrainsMono from "./assets/fonts/JetBrainsMono-Regular.ttf";
import errorCSS from "./Error.module.css";
import errorRyo from "./assets/ryo.webp";
import { PostHogProvider } from "./providers/PostHogProvider";
export const links: Route.LinksFunction = () => [
  {
    rel: "icon",
    href: "/favicon.ico",
    type: "image/x-icon",
  },
  {
    rel: "preload",
    href: satoshiFont,
    as: "font",
    type: "font/woff2",
    crossOrigin: "anonymous",
  },
  {
    rel: "preload",
    href: satoshiFontBold,
    as: "font",
    type: "font/woff2",
    crossOrigin: "anonymous",
  },
  {
    rel: "preload",
    href: satoshiFontMedium,
    as: "font",
    type: "font/woff2",
    crossOrigin: "anonymous",
  },
  {
    rel: "preload",
    href: satoshiFontBlack,
    as: "font",
    type: "font/woff2",
    crossOrigin: "anonymous",
  },
  {
    rel: "preload",
    href: jetbrainsMono,
    as: "font",
    type: "font/woff2",
    crossOrigin: "anonymous",
  },
];
export function meta({}: Route.MetaArgs) {
  return [
    { title: "ScottyLabs" },
    { name: "description", content: "The landing page for Scottylabs" },
  ];
}
export function headers(_: Route.HeadersArgs) {
  return {
    "Vercel-CDN-Cache-Control": "max-age=31556952", // 1 year
  };
}
export function Layout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <Meta />
        <Links />
      </head>
      <body>
        <StrictMode>
          <PostHogProvider>
            {children}

            <ScrollRestoration />
            <Scripts />
          </PostHogProvider>
        </StrictMode>
      </body>
    </html>
  );
}

export default function App() {
  return <Outlet />;
}

export function ErrorBoundary({ error }: Route.ErrorBoundaryProps) {
  let message = "Oops!";
  let details = "An unexpected error occurred.";
  let stack: string | undefined;

  if (isRouteErrorResponse(error)) {
    message = error.status === 404 ? "404" : "Error";
    details =
      error.status === 404
        ? "The requested page could not be found."
        : error.statusText || details;
  } else if (import.meta.env.DEV && error && error instanceof Error) {
    details = error.message;
    stack = error.stack;
  }

  return (
    <div className={errorCSS["error-container"]}>
      <h1 className={errorCSS["error-message"]}>{message}</h1>
      <p className={errorCSS["error-details"]}>{details}</p>
      {stack && (
        <pre className={errorCSS["error-stack"]}>
          <code>{stack}</code>
        </pre>
      )}
      <img src={errorRyo} height={400} />
      <br />
      <Link to={"/"} className={errorCSS["error-go-home"]}>
        <button>Go home</button>
      </Link>
    </div>
  );
}
