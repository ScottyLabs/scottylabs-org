// Project pages are not prerendered: projects are added and renamed after the site is built. The site service answers /projects/<slug>/ with 200.html (the title and description already in its head) and the page loads the project in the browser.
export const prerender = false;
export const ssr = false;
