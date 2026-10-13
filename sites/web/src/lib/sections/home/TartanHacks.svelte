<script lang="ts">
  import tartanhacksLogo from "#lib/assets/tartanhacks-logo.svg";
  import { getAllImageLinksInAssetDirectory } from "#lib/utils/files.ts";
  import ImageCarousel from "#lib/components/ImageCarousel.svelte";

  function getRandomLetter(targetChar: string, chanceForTarget: number): string {
    if (Math.random() <= chanceForTarget) return targetChar;
    const characters = "abcdefghijklmnopqrstuvwxyz ";
    const randomChar = characters[Math.floor(Math.random() * characters.length) % characters.length];
    if (randomChar === targetChar) return getRandomLetter(targetChar, 0);
    return randomChar;
  }
  const alternateNamesForParticipants = [
    "boba recipients",
    "midnight snackers",
    "morning yogis",
    "hackers",
  ];
  function generateIntermediateWord(prevWord: string, newWord: string, frontierIndex: number) {
    const len = Math.max(prevWord.length, newWord.length);
    const LETTER_CHANGE_WINDOW_SIZE = 1;

    let intermediateWord = "";
    for (let i = 0; i < len; i++) {
      if (i <= frontierIndex - LETTER_CHANGE_WINDOW_SIZE) {
        intermediateWord += newWord[i] || " ";
      } else if (i <= frontierIndex) {
        // a shorter new word blanks out its extra characters
        intermediateWord += getRandomLetter(newWord[i] || " ", newWord[i] === undefined ? 1 : 0);
      } else {
        intermediateWord += prevWord[i] || " ";
      }
    }
    return intermediateWord;
  }

  function scrambleWords(node: HTMLElement) {
    let wordIndex = 0;
    const setDisplayedWord = (word: string) => {
      node.textContent = word.replaceAll(" ", "\u00A0");
    };

    // sets up loop for transitioning in-between words
    const transitionToWord = () => {
      const startTime = Date.now();
      const wordToTransitionTo = alternateNamesForParticipants[wordIndex];
      const prevWord =
        alternateNamesForParticipants[
          (wordIndex - 1 + alternateNamesForParticipants.length) %
            alternateNamesForParticipants.length
        ];
      let finished = false;
      const intervalId = setInterval(() => {
        const frontierIndex = Math.floor((Date.now() - startTime) / 100);
        const intermediateState = generateIntermediateWord(
          prevWord,
          wordToTransitionTo,
          frontierIndex,
        );
        setDisplayedWord(intermediateState.trimEnd());
        if (intermediateState.trimEnd() === wordToTransitionTo) {
          finished = true;
          clearInterval(intervalId);
        }
      }, 30);
      return () => {
        if (!finished) {
          // we should've finished the transition though
          clearInterval(intervalId);
        }
      };
    };

    let stopTransition = transitionToWord();
    const intervalId = setInterval(() => {
      wordIndex = (wordIndex + 1) % alternateNamesForParticipants.length;
      stopTransition();
      stopTransition = transitionToWord();
    }, 4000);
    return () => {
      clearInterval(intervalId);
      stopTransition();
    };
  }
</script>

{#snippet statTile()}
  <div class="stat-box">
    <div>
      <div class="stat-box__number">700+</div>
      <div class="stat-box__description" {@attach scrambleWords}>
        {alternateNamesForParticipants[0].replaceAll(" ", "\u00A0")}
      </div>
    </div>
  </div>
{/snippet}

<section class="main-container">
  <div class="centered-section">
    <h1 class="header">We host Pittsburgh&apos;s largest annual hackathon,</h1>
    <img class="tartanhacks-logo" src={tartanhacksLogo} alt="" />
  </div>
  <div class="carousel-container carousel-container--first">
    <ImageCarousel
      heightPx={150}
      speedPxPerSecond={50}
      imageLinks={getAllImageLinksInAssetDirectory("tartanhacks-photos")}
      gapPx={15}
    />
  </div>
  <div class="carousel-container">
    <ImageCarousel
      heightPx={200}
      speedPxPerSecond={-30}
      imageLinks={getAllImageLinksInAssetDirectory("tartanhacks-photos")}
      gapPx={15}
      periodicTileInsert={statTile}
    />
  </div>
</section>

<style>
  .main-container {
    margin-top: 100px;
    background-color: var(--black-100);
    padding-top: 50px;
    padding-bottom: 75px;
  }
  .header {
    color: var(--black-900);
    text-align: center;
    margin: auto;
  }
  .tartanhacks-logo {
    margin: 40px auto 0;
    display: block;
    height: 80px;
    max-width: 100%;
    object-fit: contain;
  }
  .stat-box {
    background-color: #120902;
    border: 5px solid #90662e;
    color: #fdc274;
    display: flex;
    align-items: center;
    border-radius: 10px;
    padding-left: 30px;
    height: 100%;
    box-sizing: border-box;
    width: 370px;
    @media (max-width: 768px) {
      width: 280px;
      padding-left: 20px;
    }
  }
  .stat-box__number {
    font-size: 6rem;
    @media (max-width: 768px) {
      font-size: 4.5rem;
    }
    font-weight: 700;
    line-height: 1.1;
  }
  .stat-box__description {
    font-size: 1.875rem;
    @media (max-width: 768px) {
      font-size: 1.375rem;
    }
    font-weight: 700;
    font-family: "JetBrainsMono";
    text-wrap: nowrap;
  }
  .carousel-container {
    margin-top: 30px;
    &.carousel-container--first {
      margin-top: 60px;
    }
  }
</style>
