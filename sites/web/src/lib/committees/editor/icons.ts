// The icon for each block type, in the library, the outline and the phone list.

import ChartColumn from "@lucide/svelte/icons/chart-column";
import Calendar from "@lucide/svelte/icons/calendar";
import Image from "@lucide/svelte/icons/image";
import Images from "@lucide/svelte/icons/images";
import Megaphone from "@lucide/svelte/icons/megaphone";
import PanelsTopLeft from "@lucide/svelte/icons/panels-top-left";
import Quote from "@lucide/svelte/icons/quote";
import SquareKanban from "@lucide/svelte/icons/square-kanban";
import Type from "@lucide/svelte/icons/type";
import Users from "@lucide/svelte/icons/users";
import Video from "@lucide/svelte/icons/video";
import type { BlockType } from "../../api/types.ts";
import type { IconComponent } from "../../portal/ui/types.ts";

export const BLOCK_ICONS: Readonly<Record<BlockType, IconComponent>> = {
  cover: PanelsTopLeft,
  text: Type,
  image: Image,
  gallery: Images,
  video: Video,
  stats: ChartColumn,
  teams: Users,
  projects: SquareKanban,
  quote: Quote,
  work_sessions: Calendar,
  cta: Megaphone,
};
