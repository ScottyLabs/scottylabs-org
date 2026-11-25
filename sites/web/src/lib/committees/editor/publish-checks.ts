// What must be fixed before a page can be published, checked in the browser so the publish dialog can list it. The service checks the same rules (blocks::check_for_publishing).

import type { Block, CommitteePageDoc, ImageRef } from "../../api/types.ts";
import { blockName } from "../blocks.ts";
import { LIMITS } from "../limits.ts";
import { videoEmbed } from "../video.ts";

export interface PublishProblem {
  /** The block to fix, or null for the page settings. */
  blockId: string | null;
  /** Field path inside the block, as the service names it. */
  field: string;
  message: string;
}

function needsAlt(image: ImageRef | null): boolean {
  return image !== null && image.alt.trim() === "";
}

function blockProblems(block: Block, index: number): PublishProblem[] {
  const name = blockName(block);
  const problem = (field: string, message: string): PublishProblem => ({
    blockId: block.id,
    field,
    message,
  });
  switch (block.type) {
    case "cover":
      if (block.props.image === null)
        return [problem("image", "Add a cover picture in the page settings.")];
      return needsAlt(block.props.image)
        ? [problem("image.alt", "Describe the cover picture for people using screen readers.")]
        : [];
    case "image":
      if (block.props.image === null)
        return [problem("image", `Choose a photo for ${name} ${index}, or remove the block.`)];
      return needsAlt(block.props.image)
        ? [
            problem(
              "image.alt",
              `Describe the photo in ${name} ${index} for people using screen readers.`,
            ),
          ]
        : [];
    case "gallery": {
      const missing = block.props.images.flatMap((image, position) =>
        needsAlt(image)
          ? [
              problem(
                `images.${position}.alt`,
                `Describe photo ${position + 1} in the gallery for people using screen readers.`,
              ),
            ]
          : [],
      );
      return block.props.images.length < LIMITS.minGallery
        ? [
            problem(
              "images",
              `Add at least ${LIMITS.minGallery} photos to the gallery, or remove it.`,
            ),
            ...missing,
          ]
        : missing;
    }
    case "video": {
      const list: PublishProblem[] = [];
      if (block.props.url.trim() === "")
        list.push(problem("url", "Add a YouTube or Vimeo link to the video, or remove it."));
      else if (videoEmbed(block.props.url) === null)
        list.push(
          problem("url", "Use a link to one YouTube or Vimeo video, not a channel or playlist."),
        );
      if (needsAlt(block.props.poster)) {
        list.push(
          problem(
            "poster.alt",
            "Describe the video\u2019s poster picture for people using screen readers.",
          ),
        );
      }
      return list;
    }
    case "teams":
      return block.props.teams.length === 0
        ? [problem("teams", "Add at least one team, or remove the Teams block.")]
        : [];
    case "stats":
      return block.props.items.flatMap((item, position) =>
        item.value.trim() === ""
          ? [
              problem(
                `items.${position}.value`,
                `Fill in number ${position + 1} in Stats, or remove it.`,
              ),
            ]
          : [],
      );
    case "text":
    case "projects":
    case "quote":
    case "work_sessions":
    case "cta":
      return [];
    default: {
      const unknown: never = block;
      throw new Error(`There is no block type for ${JSON.stringify(unknown)}.`);
    }
  }
}

/** Every problem that would stop the page from publishing, in page order. */
export function publishProblems(doc: CommitteePageDoc): PublishProblem[] {
  let images = 0;
  return doc.blocks.flatMap((block) => {
    if (block.type === "image") images += 1;
    return blockProblems(block, images);
  });
}
