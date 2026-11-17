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
import bobyard from "../../assets/sponsors-page/sponsors/Special Partner/Bobyard.png";
import fada from "../../assets/sponsors-page/sponsors/Special Partner/F-ADA.png";
import gm from "../../assets/sponsors-page/sponsors/Special Partner/GM.webp";
import specialPartnerMechanize from "../../assets/sponsors-page/sponsors/Special Partner/Mechanize.png";
import redBull from "../../assets/sponsors-page/sponsors/Special Partner/RedBull.png";
import specialPartnerSalesforce from "../../assets/sponsors-page/sponsors/Special Partner/Salesforce.webp";
import uber from "../../assets/sponsors-page/sponsors/Special Partner/Uber.png";
import appLovin from "../../assets/sponsors-page/sponsors/Title/AppLovin.png";

export type SponsorTypes = "lowest" | "partner" | "premier";

export const sponsors = [
  { imageUrl: capitalOne, type: "premier", websiteUrl: "https://www.capitalone.com/" },
  { imageUrl: doordash, type: "premier", websiteUrl: "https://www.doordash.com/" },
  { imageUrl: hrt, type: "premier", websiteUrl: "https://www.hudsonrivertrading.com/" },
  { imageUrl: mechanize, type: "premier", websiteUrl: "https://mechanize.work/" },
  { imageUrl: ripple, type: "premier", websiteUrl: "https://ripple.com/" },
  { imageUrl: salesforce, type: "premier", websiteUrl: "https://www.salesforce.com/" },
  { imageUrl: accenture, type: "lowest", websiteUrl: "https://www.accenture.com/" },
  { imageUrl: anduril, type: "lowest", websiteUrl: "https://www.anduril.com/" },
  { imageUrl: citadel, type: "lowest", websiteUrl: "https://www.citadel.com/" },
  { imageUrl: deshaw, type: "lowest", websiteUrl: "https://www.deshaw.com/" },
  { imageUrl: decagon, type: "lowest", websiteUrl: "https://decagon.ai/" },
  { imageUrl: optiver, type: "lowest", websiteUrl: "https://optiver.com/" },
  { imageUrl: perplexity, type: "lowest", websiteUrl: "https://www.perplexity.ai/" },
  { imageUrl: recruitHoldings, type: "lowest", websiteUrl: "https://recruit-holdings.com/" },
  { imageUrl: runpod, type: "lowest", websiteUrl: "https://www.runpod.io/" },
  { imageUrl: sandiaNationalLaboratories, type: "lowest", websiteUrl: "https://www.sandia.gov/" },
  { imageUrl: bobyard, type: "partner", websiteUrl: "https://www.bobyard.com/" },
  { imageUrl: fada, type: "partner", websiteUrl: "https://www.f-ada.com/" },
  { imageUrl: gm, type: "partner", websiteUrl: "https://www.gm.com/" },
  { imageUrl: specialPartnerMechanize, type: "partner", websiteUrl: "https://mechanize.work/" },
  { imageUrl: redBull, type: "partner", websiteUrl: "https://www.redbull.com/" },
  { imageUrl: specialPartnerSalesforce, type: "partner", websiteUrl: "https://www.salesforce.com/" },
  { imageUrl: uber, type: "partner", websiteUrl: "https://www.uber.com/" },
] satisfies { imageUrl: string; type: SponsorTypes; websiteUrl: string }[];
