import { HISTORY, type HistoryBlock } from "../data/history";
import { HistoryArticle } from "../sections/history/article";
import { HistoryCarousel } from "../sections/history/carousel";

type YearGroup = {
  year: string;
  blocks: HistoryBlock[];
};

const groupByYear = (blocks: HistoryBlock[]): YearGroup[] => {
  const groups: YearGroup[] = [];
  for (const block of blocks) {
    const last = groups[groups.length - 1];
    if (last && last.year === block.year) {
      last.blocks.push(block);
    } else {
      groups.push({ year: block.year, blocks: [block] });
    }
  }
  return groups;
};

export default function OverTheYears() {
  const groups = groupByYear(HISTORY);

  return (
    <section
      style={{ padding: "48px 24px", maxWidth: "1400px", margin: "0 auto" }}
    >
      <h1 style={{ fontSize: "32px", margin: "0 0 24px 0", color: "#07123b" }}>
        ScottyLabs Over the Years
      </h1>
      <div
        style={{
          borderRadius: "20px",
          border: "1px solid #B0B0B0",
          background: "#F2F3F7",
          padding: "48px",
        }}
      >
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "48px",
            fontFamily: "Satoshi, sans-serif",
            color: "#07123b",
          }}
        >
          {groups.map((group) => (
            <div
              key={group.year}
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "16px",
              }}
            >
              <div
                style={{
                  fontWeight: 700,
                  fontSize: "20px",
                  color: "#07123b",
                }}
              >
                {group.year}
              </div>
              {group.blocks.map((block) =>
                block.type === "carousel" ? (
                  <HistoryCarousel
                    key={`carousel-${block.photos[0].filename}`}
                    photos={block.photos}
                  />
                ) : (
                  <HistoryArticle
                    key={`article-${block.url}`}
                    article={block}
                  />
                ),
              )}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
