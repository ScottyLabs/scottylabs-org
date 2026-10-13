import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import css from "./Projects.module.css";
import { AnimatePresence, motion } from "motion/react";
import { getAllImageLinksInAssetDirectory } from "../../utils/files";
import clsx from "clsx";
import externalLinkIcon from "../../assets/icons/go-to-icon.svg";

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

function IconDecoration() {
  const designIcons = getAllImageLinksInAssetDirectory("design-app-icons");
  const codeIcons = getAllImageLinksInAssetDirectory("code-app-icons");
  return (
    <div>
      {designIcons.slice(0, 4).map((icon) => (
        <motion.img
          drag
          dragSnapToOrigin
          className={clsx(
            css["tool-logo-slot"],
            icon.includes("/ai") && css["tool-logo-slot--ai-san"],
          )}
          src={icon}
          alt=""
          key={icon}
        />
      ))}
      {codeIcons.slice(0, 4).map((icon) => (
        <motion.img
          drag
          dragSnapToOrigin
          className={css["tool-logo-slot"]}
          src={icon}
          alt=""
          key={icon}
        />
      ))}
    </div>
  );
}
function DesignBoxDecoration() {
  return (
    <div className={css["decoration"]}>
      <div className={css["decoration__line"]} />
      <div className={css["decoration__line"]} />
      <div className={css["decoration__line"]} />
      <div className={css["decoration__line"]} />
      <div className={css["decoration__square"]} />
      <div className={css["decoration__square"]} />
      <div className={css["decoration__square"]} />
      <div className={css["decoration__square"]} />
    </div>
  );
}

function ProjectTab({
  name,
  assetFolder,
  isSelected,
  onClick,
}: {
  name: string;
  assetFolder: string;
  isSelected: boolean;
  onClick: (position: [number, number]) => unknown;
}) {
  const tabRef = useRef<HTMLButtonElement | null>(null);
  const updateBottomBorderPosition = useCallback(() => {
    onClick(
      tabRef.current
        ? [
            tabRef.current.getBoundingClientRect().left,
            tabRef.current.getBoundingClientRect().right,
          ]
        : [0, 0],
    );
  }, []); // onClick doesn't really "change", and if we include it we get stuck in an infinite render loop when we call onClick, since every re-render of the parent component produces a "new version" of the onClick function
  useEffect(() => {
    const onResize = () => {
      if (isSelected) {
        updateBottomBorderPosition();
      }
    };
    window.addEventListener("resize", onResize);
    onResize();
    return () => window.removeEventListener("resize", onResize);
  }, [updateBottomBorderPosition, isSelected]);
  return (
    <button
      className={css["tab"]}
      role="tab"
      aria-selected={isSelected ? "true" : "false"}
      onClick={updateBottomBorderPosition}
      key={assetFolder}
      ref={tabRef}
    >
      <img
        className={css["tab__image"]}
        src={`/projects/${assetFolder}/icon.png`}
        onLoad={() => {
          if (isSelected) updateBottomBorderPosition();
        }}
        alt=""
      />
      <div className={css["tab__name"]}>{name}</div>
    </button>
  );
}
function ProjectPreviews() {
  const [selectedProjectIndex, setSelectedProjectIndex] = useState(0);
  const [bottomBorderCoords, setBottomBorderCoords] = useState<
    [number, number]
  >([0, 0]);
  const containerRef = useRef<HTMLDivElement | null>(null);

  return (
    <>
      <div
        className={css["project-tabs-container"]}
        style={
          {
            "--border-left-px": bottomBorderCoords[0] + "px",
            "--border-right-px": bottomBorderCoords[1] + "px",
          } as CSSProperties
        }
        ref={containerRef}
      >
        <ul className={css["project-tabs"]} role="tablist">
          {featuredProjects.map(({ name, assetFolder }, i) => {
            return (
              <ProjectTab
                name={name}
                assetFolder={assetFolder}
                isSelected={i === selectedProjectIndex}
                key={assetFolder}
                onClick={(coords: [number, number]) => {
                  const offsetPx =
                    containerRef.current?.getBoundingClientRect().left ?? 0;
                  setSelectedProjectIndex(i);
                  setBottomBorderCoords([
                    coords[0] - offsetPx,
                    coords[1] - offsetPx,
                  ]);
                }}
              />
            );
          })}
        </ul>
      </div>
      <div className={css["panel-container"]}>
        <AnimatePresence>
          <motion.div
            className={css["panel"]}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            // we want to keep the old panel visible for a while for a smoother transition,
            // but setting opacity to 1 destroys the panel immediately
            exit={{ opacity: 0.99 }}
            key={selectedProjectIndex}
          >
            <img
              className={css["panel__img"]}
              src={`/projects/${featuredProjects[selectedProjectIndex].assetFolder}/main.png`}
              alt=""
            />
            <div className={css["panel__details"]}>
              <span className={css["panel__details__description"]}>
                {featuredProjects[selectedProjectIndex].description}
              </span>
              <div className={css["panel__details__footer"]}>
                <a
                  href={featuredProjects[selectedProjectIndex].link}
                  target="_blank"
                >
                  <img src={externalLinkIcon} alt="" />
                  Visit
                </a>
              </div>
            </div>
          </motion.div>
        </AnimatePresence>
      </div>
    </>
  );
}
export default function Projects() {
  return (
    <section className="centered-section">
      <div className={css["title-section"]}>
        <h1 className={css["title-section__header"]}>
          From{" "}
          <span className={css["title-section__header__design-text"]}>
            Design
            <DesignBoxDecoration />
          </span>{" "}
          to{" "}
          <span className={css["title-section__header__code-block"]}>
            {"<"}
          </span>
          Development
          <span className={css["title-section__header__code-block"]}>
            {"/>"}
          </span>
          <IconDecoration />
        </h1>
        <p className={css["title-section__desc"]}>
          We bring to life a variety of tech services geared towards improving
          the CMU campus experience and inspiring the community!
        </p>
      </div>
      <ProjectPreviews />
    </section>
  );
}
