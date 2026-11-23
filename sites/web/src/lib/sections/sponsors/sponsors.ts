import accenture from "../../assets/sponsors-page/sponsors/Partner/Accenture.webp";
import anduril from "../../assets/sponsors-page/sponsors/Partner/Anduril.png";
import citadel from "../../assets/sponsors-page/sponsors/Partner/Citadel.png";
import deshaw from "../../assets/sponsors-page/sponsors/Partner/DEShaw&Co.webp";
import decagon from "../../assets/sponsors-page/sponsors/Partner/Decagon.jpg";
import optiver from "../../assets/sponsors-page/sponsors/Partner/Optiver.jpg";
import perplexity from "../../assets/sponsors-page/sponsors/Partner/Perplexity.jpg";
import recruitHoldings from "../../assets/sponsors-page/sponsors/Partner/Recruit_Holdings.png";
import runpod from "../../assets/sponsors-page/sponsors/Partner/RunPod.png";
import sandiaNationalLaboratories from "../../assets/sponsors-page/sponsors/Partner/Sandia_National_Laboratories.webp";
import capitalOne from "../../assets/sponsors-page/sponsors/Premier/CapitalOne.webp";
import doordash from "../../assets/sponsors-page/sponsors/Premier/Doordash.jpg";
import hrt from "../../assets/sponsors-page/sponsors/Premier/HRT.png";
import mechanize from "../../assets/sponsors-page/sponsors/Premier/Mechanize.png";
import ripple from "../../assets/sponsors-page/sponsors/Premier/Ripple.png";
import salesforce from "../../assets/sponsors-page/sponsors/Premier/Salesforce.webp";
import jumptrading from "../../assets/sponsors-page/sponsors/Premier/Jump_Trading.png";

export type SponsorTypes = "lowest" | "partner" | "premier";

export const sponsors: { imageUrl: string; type: SponsorTypes; websiteUrl: string }[] = [
  { imageUrl: capitalOne, type: "premier", websiteUrl: "https://www.capitalone.com/" },
  { imageUrl: doordash, type: "premier", websiteUrl: "https://www.doordash.com/" },
  { imageUrl: hrt, type: "premier", websiteUrl: "https://www.hudsonrivertrading.com/" },
  { imageUrl: mechanize, type: "premier", websiteUrl: "https://mechanize.work/" },
  { imageUrl: ripple, type: "premier", websiteUrl: "https://ripple.com/" },
  { imageUrl: salesforce, type: "premier", websiteUrl: "https://www.salesforce.com/" },
  { imageUrl: jumptrading, type: "premier", websiteUrl: "https://www.jumptrading.com/" },
  { imageUrl: accenture, type: "partner", websiteUrl: "https://www.accenture.com/" },
  { imageUrl: anduril, type: "partner", websiteUrl: "https://www.anduril.com/" },
  { imageUrl: citadel, type: "partner", websiteUrl: "https://www.citadel.com/" },
  { imageUrl: deshaw, type: "partner", websiteUrl: "https://www.deshaw.com/" },
  { imageUrl: decagon, type: "partner", websiteUrl: "https://decagon.ai/" },
  { imageUrl: optiver, type: "partner", websiteUrl: "https://optiver.com/" },
  { imageUrl: perplexity, type: "partner", websiteUrl: "https://www.perplexity.ai/" },
  { imageUrl: recruitHoldings, type: "partner", websiteUrl: "https://recruit-holdings.com/" },
  { imageUrl: runpod, type: "partner", websiteUrl: "https://www.runpod.io/" },
  { imageUrl: sandiaNationalLaboratories, type: "partner", websiteUrl: "https://www.sandia.gov/" },
];
