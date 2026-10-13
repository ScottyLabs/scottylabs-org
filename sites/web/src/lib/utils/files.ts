// import.meta.glob only accepts string literals
const IMAGE_DIRECTORIES = {
  "carousel-images": import.meta.glob<string>("../assets/irl/carousel/*", {
    eager: true,
    query: "url",
    import: "default",
  }),
  "design-app-icons": import.meta.glob<string>("../assets/icons/apps/design/*", {
    eager: true,
    query: "url",
    import: "default",
  }),
  "code-app-icons": import.meta.glob<string>("../assets/icons/apps/code/*", {
    eager: true,
    query: "url",
    import: "default",
  }),
  "tartanhacks-photos": import.meta.glob<string>("../assets/irl/tartanhacks/*", {
    eager: true,
    query: "url",
    import: "default",
  }),
};

export function getAllImageLinksInAssetDirectory(dir: keyof typeof IMAGE_DIRECTORIES): string[] {
  return Object.values(IMAGE_DIRECTORIES[dir]);
}
