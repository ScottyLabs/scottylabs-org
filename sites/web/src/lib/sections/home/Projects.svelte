<script lang="ts">
  import { untrack } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { getAllImageLinksInAssetDirectory } from "#lib/utils/files.ts";
  import externalLinkIcon from "#lib/assets/icons/go-to-icon.svg";

  const featuredProjects = [
    {
      name: "CMU Courses",
      assetFolder: "cmu-courses",
      description: `CMU Courses is a course browser built by and for CMU students.
          Bringing together course information, schedules and FCE data, it makes
          it possible for CMU students to plan their semesters and browse for
          courses.`,
      link: "https://www.cmucourses.com/",
    },
    {
      name: "CMU Eats",
      assetFolder: "cmu-eats",
      description:
        "CMUEats is your one-stop-shop for finding out what campus dining locations are open right now.",
      link: "https://cmueats.com/",
    },
    {
      name: "CMU Maps",
      assetFolder: "cmu-maps",
      description: "Providing indoor maps and navigation for the CMU campus.",
      link: "https://cmumaps.com/",
    },
  ];

  const designIcons = getAllImageLinksInAssetDirectory("design-app-icons");
  const codeIcons = getAllImageLinksInAssetDirectory("code-app-icons");

  const DRAG_DISTANCE_THRESHOLD_PX = 3;
  const SPRING_STIFFNESS = 200;
  const SPRING_DAMPING = 40;
  const SPRING_REST_DELTA = 1;
  const SPRING_REST_SPEED = 10;

  function springToOrigin(from: number) {
    const initialDelta = -from;
    const dampingRatio = SPRING_DAMPING / (2 * Math.sqrt(SPRING_STIFFNESS));
    const undampedAngularFreq = Math.sqrt(SPRING_STIFFNESS) / 1000;
    const dampedAngularFreq = undampedAngularFreq * Math.sqrt(dampingRatio * dampingRatio - 1);
    const decay = dampingRatio * undampedAngularFreq;
    const P = (decay * initialDelta) / dampedAngularFreq;
    const sinhCoeff = decay * P - initialDelta * dampedAngularFreq;
    const coshCoeff = decay * initialDelta - P * dampedAngularFreq;
    return (t: number) => {
      const envelope = Math.exp(-decay * t);
      const freqForT = Math.min(dampedAngularFreq * t, 300);
      const value =
        -(
          envelope *
          (decay * initialDelta * Math.sinh(freqForT) +
            dampedAngularFreq * initialDelta * Math.cosh(freqForT))
        ) / dampedAngularFreq;
      const velocity =
        envelope * (sinhCoeff * Math.sinh(freqForT) + coshCoeff * Math.cosh(freqForT)) * 1000;
      const done = Math.abs(velocity) <= SPRING_REST_SPEED && Math.abs(value) <= SPRING_REST_DELTA;
      return done ? 0 : value;
    };
  }

  const dragSnapToOrigin: Attachment<HTMLElement> = (node) => {
    let x = 0;
    let y = 0;
    let frameId: number | undefined;
    let removePointerListeners: (() => void) | undefined;

    const render = () => {
      node.style.transform = x === 0 && y === 0 ? "none" : `translateX(${x}px) translateY(${y}px)`;
    };
    const stopAnimation = () => {
      if (frameId !== undefined) cancelAnimationFrame(frameId);
      frameId = undefined;
    };
    const startAnimation = () => {
      const resolveX = springToOrigin(x);
      const resolveY = springToOrigin(y);
      let startTime: number | undefined;
      const step = (time: number) => {
        startTime ??= time;
        x = resolveX(time - startTime);
        y = resolveY(time - startTime);
        render();
        frameId = x === 0 && y === 0 ? undefined : requestAnimationFrame(step);
      };
      frameId = requestAnimationFrame(step);
    };

    const onPointerDown = (downEvent: PointerEvent) => {
      if (downEvent.pointerType === "mouse" ? downEvent.button > 0 : !downEvent.isPrimary) return;
      removePointerListeners?.();
      stopAnimation();
      const originX = x;
      const originY = y;
      let isDragging = false;

      const onPointerMove = (event: PointerEvent) => {
        if (event.pointerId !== downEvent.pointerId) return;
        const offsetX = event.pageX - downEvent.pageX;
        const offsetY = event.pageY - downEvent.pageY;
        if (!isDragging && Math.hypot(offsetX, offsetY) < DRAG_DISTANCE_THRESHOLD_PX) return;
        isDragging = true;
        x = originX + offsetX;
        y = originY + offsetY;
        render();
      };
      const onPointerUp = (event: PointerEvent) => {
        if (event.pointerId !== downEvent.pointerId) return;
        removePointerListeners?.();
        if (x !== 0 || y !== 0) startAnimation();
      };
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
      window.addEventListener("pointercancel", onPointerUp);
      removePointerListeners = () => {
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", onPointerUp);
        window.removeEventListener("pointercancel", onPointerUp);
        removePointerListeners = undefined;
      };
    };

    node.addEventListener("pointerdown", onPointerDown);
    return () => {
      node.removeEventListener("pointerdown", onPointerDown);
      removePointerListeners?.();
      stopAnimation();
    };
  };

  let selectedProjectIndex = $state(0);
  let bottomBorderCoords = $state<[number, number]>([0, 0]);
  let containerEl: HTMLDivElement | undefined = $state();
  const tabEls: HTMLButtonElement[] = $state([]);

  type Panel = { index: number; exiting: boolean; exitComplete: boolean };
  let panels: Panel[] = $state([{ index: 0, exiting: false, exitComplete: false }]);

  function completeExit(panel: Panel) {
    panel.exitComplete = true;
    if (panels.every((p) => !p.exiting || p.exitComplete)) {
      panels = panels.filter((p) => !p.exiting);
    }
  }

  const panelOpacity =
    (panel: Panel): Attachment<HTMLElement> =>
    (node) => {
      const exiting = panel.exiting;
      const target = exiting ? 0.99 : 1;
      const animation = node.animate(
        [{ opacity: getComputedStyle(node).opacity }, { opacity: target }],
        { duration: 300, easing: "cubic-bezier(0.25, 0.1, 0.35, 1)" },
      );
      node.style.opacity = `${target}`;
      animation.onfinish = () => {
        if (exiting) completeExit(panel);
      };
      return () => {
        animation.onfinish = null;
        if (node.isConnected && animation.playState === "running") animation.commitStyles();
        animation.cancel();
      };
    };

  function selectTab(index: number) {
    const rect = tabEls[index]?.getBoundingClientRect();
    const coords = rect === undefined ? [0, 0] : [rect.left, rect.right];
    const offsetPx = containerEl?.getBoundingClientRect().left ?? 0;
    if (index !== selectedProjectIndex) {
      for (const panel of panels) {
        panel.exiting = panel.index !== index;
        if (!panel.exiting) panel.exitComplete = false;
      }
      if (!panels.some((panel) => panel.index === index)) {
        panels.push({ index, exiting: false, exitComplete: false });
      }
    }
    selectedProjectIndex = index;
    bottomBorderCoords = [coords[0] - offsetPx, coords[1] - offsetPx];
  }

  $effect(() => {
    const index = selectedProjectIndex;
    const onResize = () => {
      selectTab(index);
    };
    window.addEventListener("resize", onResize);
    untrack(onResize);
    return () => {
      window.removeEventListener("resize", onResize);
    };
  });
</script>

{#snippet toolLogo(icon: string, isAiSan: boolean)}
  <img
    class={["tool-logo-slot", isAiSan && "tool-logo-slot--ai-san"]}
    src={icon}
    alt=""
    draggable="false"
    style="user-select: none; -webkit-user-select: none; -webkit-touch-callout: none; touch-action: none"
    {@attach dragSnapToOrigin}
  />
{/snippet}

<section class="centered-section">
  <div class="title-section">
    <h1 class="title-section__header">
      From <span class="title-section__header__design-text"
        >Design<div class="decoration">
          <div class="decoration__line"></div>
          <div class="decoration__line"></div>
          <div class="decoration__line"></div>
          <div class="decoration__line"></div>
          <div class="decoration__square"></div>
          <div class="decoration__square"></div>
          <div class="decoration__square"></div>
          <div class="decoration__square"></div>
        </div></span
      >
      to <span class="title-section__header__code-block">{"<"}</span>Development<span
        class="title-section__header__code-block">{"/>"}</span
      ><div>
        {#each designIcons.slice(0, 4) as icon (icon)}
          {@render toolLogo(icon, icon.includes("/ai"))}
        {/each}
        {#each codeIcons.slice(0, 4) as icon (icon)}
          {@render toolLogo(icon, false)}
        {/each}
      </div>
    </h1>
    <p class="title-section__desc">
      We bring to life a variety of tech services geared towards improving the CMU campus experience
      and inspiring the community!
    </p>
  </div>
  <div
    class="project-tabs-container"
    style:--border-left-px="{bottomBorderCoords[0]}px"
    style:--border-right-px="{bottomBorderCoords[1]}px"
    bind:this={containerEl}
  >
    <ul class="project-tabs" role="tablist">
      {#each featuredProjects as { name, assetFolder }, i (assetFolder)}
        <button
          class="tab"
          role="tab"
          aria-selected={i === selectedProjectIndex ? "true" : "false"}
          onclick={() => selectTab(i)}
          bind:this={tabEls[i]}
        >
          <img
            class="tab__image"
            src="/projects/{assetFolder}/icon.png"
            onload={() => {
              if (i === selectedProjectIndex) selectTab(i);
            }}
            alt=""
          />
          <div class="tab__name">{name}</div>
        </button>
      {/each}
    </ul>
  </div>
  <div class="panel-container">
    {#each panels as panel (panel.index)}
      <div class="panel" style="opacity: 0" {@attach panelOpacity(panel)}>
        <img class="panel__img" src="/projects/{featuredProjects[panel.index].assetFolder}/main.png" alt="" />
        <div class="panel__details">
          <span class="panel__details__description">
            {featuredProjects[panel.index].description}
          </span>
          <div class="panel__details__footer">
            <a href={featuredProjects[panel.index].link} target="_blank">
              <img src={externalLinkIcon} alt="" />
              Visit
            </a>
          </div>
        </div>
      </div>
    {/each}
  </div>
</section>

<style>
  .title-section {
    text-align: center;
    margin-top: 190px;
    margin-bottom: 170px;
    @media (max-width: 768px) {
      margin-top: 130px;
      margin-bottom: 130px;
    }
    position: relative;
  }
  .title-section__header {
    color: var(--black-900);
  }
  .title-section__desc {
    color: var(--black-500);
    line-height: 1.5;
    margin-top: 20px;
    font-size: 1.5rem;
    @media (max-width: 768px) {
      font-size: var(--text-lg);
    }
    max-width: 800px;
    margin-inline: auto;
  }
  .tool-logo-slot {
    z-index: 99;
    position: absolute;
    --icon-size: 55px;
    @media (max-width: 768px) {
      --icon-size: 40px;
    }
    cursor: grab;
    will-change: transform;
    &:active {
      cursor: grabbing;
    }

    height: var(--icon-size);
    width: var(--icon-size);
    opacity: 0.6;
    &:nth-child(1) {
      left: 0;
      top: 0;
    }
    &:nth-child(2) {
      top: calc(-1 * var(--icon-size) - 30px);
      left: 250px;
    }
    &:nth-child(3) {
      bottom: calc(-1 * var(--icon-size) - 30px);
      left: 100px;
    }
    &:nth-child(4) {
      bottom: calc(-1 * var(--icon-size) - 50px);
      left: 450px;
    }
    &:nth-child(5) {
      top: calc(-1 * var(--icon-size) - 30px);
      right: 450px;
    }
    &:nth-child(6) {
      bottom: calc(-1 * var(--icon-size) - 70px);
      right: 250px;
    }
    &:nth-child(7) {
      top: calc(-1 * var(--icon-size) - 50px);
      right: 150px;
    }
    &:nth-child(8) {
      top: calc(-1 * var(--icon-size) -50px);
      right: 10px;
    }
    @media (max-width: 1024px) {
      &:nth-child(1) {
        top: calc(-1 * var(--icon-size) - 20px);
        left: 5%;
      }
      &:nth-child(2) {
        left: 30%;
      }
      &:nth-child(3) {
        left: 10%;
      }
      &:nth-child(4) {
        left: 45%;
      }
      &:nth-child(5) {
        right: 45%;
      }
      &:nth-child(6) {
        right: 15%;
      }
      &:nth-child(7) {
        right: 20%;
      }
    }
    &.tool-logo-slot--ai-san:hover {
      content: url("../../assets/ai-san.png");
    }
  }
  .title-section__header__design-text {
    position: relative;
  }
  .decoration {
    position: absolute;
    /* background-color: blue; */
    inset: 0 -5px -7px;
    z-index: -1;
    --border-color: #24a4ff;
  }
  .decoration__line {
    background-color: var(--border-color);
    position: absolute;
    &:nth-child(1) {
      inset: 0 auto 0 0;
      width: 2px;
    }
    &:nth-child(2) {
      inset: 0 0 0 auto;
      width: 2px;
    }
    &:nth-child(3) {
      inset: 0 0 auto 0;
      height: 2px;
    }
    &:nth-child(4) {
      inset: auto 0 0 0;
      height: 2px;
    }
  }
  .decoration__square {
    position: absolute;
    height: 6px;
    width: 6px;
    border: 2px solid var(--border-color);
    background-color: var(--bg-color);
    &:nth-child(5) {
      top: -4px;
      left: -4px;
    }
    &:nth-child(6) {
      top: -4px;
      right: -4px;
    }
    &:nth-child(7) {
      bottom: -4px;
      left: -4px;
    }
    &:nth-child(8) {
      bottom: -4px;
      right: -4px;
    }
  }
  .title-section__header__code-block {
    color: var(--black-300);
    font-weight: 400;
    font-family: JetBrainsMono;
  }
  .project-tabs-container {
    --border-height: 4px;
    --border-left-px: 0;
    --border-right-px: 0;
    border-bottom: var(--border-height) solid var(--black-100);
    position: relative;
    &::after {
      transition: 0.5s cubic-bezier(0.27, 1.32, 0.41, 1);
      content: "";
      position: absolute;
      inset: auto 0 calc(-1 * var(--border-height));
      height: var(--border-height);
      background: linear-gradient(90deg, #36a5f2 0%, #d28dff 47.12%, #ff8d8d 99.52%);
      padding-left: var(--border-left-px);
      padding-right: calc(100% - var(--border-right-px));
      background-clip: content-box;
    }
  }
  .project-tabs {
    margin: 0 auto;
    display: flex;
    justify-content: space-evenly;
  }
  .tab {
    display: flex;
    position: relative;
    gap: 10px;
    align-items: center;
    font-size: var(--text-xl);
    font-weight: 700;
    color: var(--black-900);
    padding: 18px 10px;
    @media (max-width: 768px) {
      flex-direction: column;
      gap: 6px;
      padding: 14px 4px;
      font-size: var(--text-base);
      text-align: center;
    }
    border-bottom: var(--border-height) solid transparent;
    margin-bottom: calc(-1 * var(--border-height));
    transition: 0.15s border-color;
    &[aria-selected="true"] {
      border-color: transparent;
      &::after {
        content: '[aria-selected="true"]';
        position: absolute;
        inset: 0 -50px auto;
        text-align: center;
        transform: translate(0, -10px);
        font-size: var(--text-xs);
        font-family: JetBrainsMono;
        color: var(--black-300);
        animation: appear 0.3s;
      }
      &:hover {
        border-color: transparent;
        &:active {
          border-color: transparent;
        }
      }
    }

    &:hover {
      border-color: var(--black-300);
      &:active {
        border-color: transparent;
      }
    }
  }
  @keyframes appear {
    from {
      transform: translate(0);
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .tab__image {
    height: 1.4em;
  }
  .panel-container {
    height: 450px;
    @media (min-height: 700px) {
      height: 550px;
    }
    @media (max-width: 768px) {
      height: 560px;
    }
    margin-top: 35px;
    position: relative;
    border-radius: 11px;
    border: 1px solid var(--black-100);
    box-shadow: 0 4px 4px 0 rgba(0, 0, 0, 0.25);
    overflow: hidden;
  }
  .panel {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-columns: 1fr 350px;
    @media (max-width: 768px) {
      grid-template-columns: 1fr;
      grid-template-rows: 220px 1fr;
    }
  }

  .panel__img {
    width: 100%;
    height: 100%;
    min-height: 0; /* prevents the image from making the grid taller than it already is */
    object-fit: cover;
    object-position: top left;
  }
  .panel__details {
    padding: 35px 25px;
    background-color: var(--blue-50);
    border-left: 5px solid var(--black-100);
    @media (max-width: 768px) {
      padding: 20px;
      border-left: none;
      border-top: 5px solid var(--black-100);
    }
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 20px;
    overflow: auto;
    & .panel__details__description {
      color: var(--black-900);
      font-size: 1.25rem;
      line-height: 1.5;
    }
    & .panel__details__footer {
      display: flex;
      align-items: center;
      gap: 25px;
      & > a {
        display: flex;
        vertical-align: middle;
        gap: 10px;
        align-content: center;
        color: var(--black-500);
        & > img {
          opacity: 50%;
        }
      }
    }
  }
</style>
