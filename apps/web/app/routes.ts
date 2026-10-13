import {
  index,
  layout,
  type RouteConfig,
  route,
} from "@react-router/dev/routes";

export default [
  layout("routes/layout.tsx", [
    index("routes/home.tsx"),
    route("/events", "routes/events.tsx"),
    route("/sponsors", "routes/sponsors.tsx"),
    route("/team", "routes/team.tsx"),
    route("/over-the-years", "routes/over-the-years.tsx"),
  ]),
] satisfies RouteConfig;
