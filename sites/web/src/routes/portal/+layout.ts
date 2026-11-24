// The portal is rendered in the browser only. The site service answers every /portal/ path with
// 200.html, and the layout checks GET /api/me before showing anything.
export const ssr = false;
export const prerender = false;
