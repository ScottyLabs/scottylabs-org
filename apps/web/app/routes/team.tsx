import { ArrowRight } from "lucide-react";
import { Link } from "react-router";
import {
  COMMITTEES,
  type Committee,
  type PersonId,
  type TeamMember,
  YEARS,
  type Year,
} from "../data/team";
import {
  type CardData,
  DirectorCard,
  TeamMemberCard,
} from "../sections/team/cards";
import {
  directorRoleFor,
  personById,
  photoUrlFor,
} from "../sections/team/lookups";

const directorCard = (id: PersonId, committee: Committee): CardData => {
  const p = personById(id);
  return {
    name: p.name,
    role: directorRoleFor(committee),
    photoUrl: photoUrlFor(id),
    href: p.linkedin,
  };
};

const memberCard = (m: TeamMember): CardData => {
  const p = personById(m.person);
  return {
    name: p.name,
    role: m.role,
    photoUrl: photoUrlFor(m.person),
    href: p.linkedin,
  };
};

export default function Team() {
  // Exact colors from Figma with 60% opacity
  const lineColors = [
    "rgba(177, 179, 233, 0.00)", // Column 0 - Director of ScottyLabs (transparent)
    "rgba(177, 179, 233, 0.60)", // Column 1 - Events
    "rgba(177, 202, 233, 0.60)", // Column 2 - Outreach
    "rgba(236, 151, 152, 0.60)", // Column 3 - Tech
    "rgba(232, 187, 174, 0.60)", // Column 4 - Labrador
    "rgba(205, 179, 210, 0.60)", // Column 5 - Design
    "rgba(179, 213, 224, 0.60)", // Column 6 - Finance
  ];

  return (
    <section
      style={{ padding: "48px 24px", maxWidth: "1400px", margin: "0 auto" }}
    >
      <h1 style={{ fontSize: "32px", margin: "0 0 24px 0", color: "#07123b" }}>
        Leadership
      </h1>
      <div
        style={{
          borderRadius: "20px",
          border: "1px solid #B0B0B0",
          background: "#F2F3F7",
          padding: "48px",
          paddingLeft: "130px",
          paddingRight: 40,
          position: "relative",
          overflowX: "auto",
          overflowY: "hidden",
        }}
      >
        {/* Content with relative positioning */}
        <div
          style={{
            position: "relative",
            zIndex: 1,
            minWidth: 1000,
          }}
        >
          {/* Vertical lines spanning the entire height */}
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "grid",
              gridTemplateColumns: "repeat(7, minmax(0, 1fr))",
              gap: "12px",
              pointerEvents: "none",
              zIndex: 0,
            }}
          >
            {COMMITTEES.map((committee, idx) => (
              <div
                key={committee}
                style={{
                  display: "flex",
                  justifyContent: "center",
                  alignItems: "flex-start",
                }}
              >
                <div
                  style={{
                    width: "25px",
                    height: "100%",
                    backgroundColor: lineColors[idx],
                    borderRadius: "9999px",
                  }}
                />
              </div>
            ))}
          </div>
          {YEARS.map((year: Year, yearIdx: number) => (
            <div
              key={year.label}
              style={{
                marginBottom: yearIdx === YEARS.length - 1 ? "0" : "60px",
                position: "relative",
              }}
            >
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(7, 1fr)",
                  gap: "12px",
                  marginBottom: "16px",
                }}
              >
                <div
                  style={{
                    position: "absolute",
                    left: "-110px",
                    top: "100px",
                    transform: "translateY(-50%)",
                    color: "#000",
                    fontFamily: "Satoshi, sans-serif",
                    fontSize: "20px",
                    fontWeight: 700,
                    lineHeight: "1",
                    whiteSpace: "nowrap",
                    zIndex: 2,
                  }}
                >
                  {year.label}
                </div>

                <div
                  style={{
                    position: "absolute",
                    top: "100px",
                    left: "-20px",
                    width: "calc(100% / 7 * 6 + (100% / 7 + 12px) / 2 + 20px)",
                    height: "1px",
                    backgroundColor: "#819AFF",
                    zIndex: 0,
                  }}
                >
                  <div
                    style={{
                      position: "absolute",
                      left: "-4px",
                      top: "-4px",
                      width: "8px",
                      height: "8px",
                      borderRadius: "50%",
                      backgroundColor: "#819AFF",
                    }}
                  />
                </div>

                {COMMITTEES.map((committee) => {
                  const id = year.directors[committee];
                  return (
                    <div
                      key={committee}
                      style={{
                        display: "flex",
                        justifyContent: "center",
                        position: "relative",
                        zIndex: 1,
                      }}
                    >
                      {id ? (
                        <DirectorCard person={directorCard(id, committee)} />
                      ) : null}
                    </div>
                  );
                })}
              </div>

              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(7, 1fr)",
                  gap: "12px",
                }}
              >
                {COMMITTEES.map((committee) => {
                  const team = year.teams[committee] ?? [];
                  return (
                    <div
                      key={committee}
                      style={{
                        display: "grid",
                        gridTemplateColumns: "repeat(2, minmax(0,1fr))",
                        gap: 8,
                        alignItems: "center",
                        height: "fit-content",
                      }}
                    >
                      {team.map((member) => (
                        <TeamMemberCard
                          key={`${member.person}-${member.role}`}
                          person={memberCard(member)}
                        />
                      ))}
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      </div>

      <div
        style={{
          marginTop: "32px",
          textAlign: "center",
          fontFamily: "Satoshi, sans-serif",
          fontSize: "14px",
          color: "#6b7280",
        }}
      >
        <Link
          to="/over-the-years"
          style={{
            color: "#819AFF",
            textDecoration: "none",
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
          }}
        >
          ScottyLabs Over the Years
          <ArrowRight size={16} />
        </Link>
      </div>
    </section>
  );
}
