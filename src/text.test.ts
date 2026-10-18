import { describe, expect, it } from "vitest";
import { polishTranscript } from "./text";

describe("polishTranscript", () => {
  it("converts spoken punctuation only for whole words", () => {
    expect(polishTranscript("Das Kommando ist gut komma danke", "neutral", "light", "")).toBe("Das Kommando ist gut, danke.");
    expect(polishTranscript("Die Punktzahl stimmt punkt", "neutral", "light", "")).toBe("Die Punktzahl stimmt.");
  });

  it("keeps times, decimals and punctuation clusters intact", () => {
    expect(polishTranscript("Treffen um 12:30 Uhr mit 3,5 Prozent", "neutral", "light", "")).toBe("Treffen um 12:30 Uhr mit 3,5 Prozent.");
    expect(polishTranscript("Wirklich?!", "neutral", "light", "")).toBe("Wirklich?!");
    expect(polishTranscript("Hallo,Welt", "neutral", "light", "")).toBe("Hallo, Welt.");
  });

  it("applies dictionary entries with umlauts at word boundaries and literal replacements", () => {
    expect(polishTranscript("das ölfass ist voll", "neutral", "light", "ölfass => Ölfass")).toBe("Das Ölfass ist voll.");
    expect(polishTranscript("kostet zehn euro", "neutral", "light", "zehn euro => $10")).toBe("Kostet $10.");
  });

  it("removes only whole-word direct repetitions", () => {
    expect(polishTranscript("ich ich gehe", "neutral", "light", "")).toBe("Ich gehe.");
    expect(polishTranscript("Kühe he", "neutral", "light", "")).toBe("Kühe he.");
    expect(polishTranscript("foo_bar bar", "neutral", "light", "")).toBe("Foo_bar bar.");
    expect(polishTranscript("foo_bar bar_baz", "neutral", "light", "")).toBe("Foo_bar bar_baz.");
  });

  it("converts spoken paragraphs and additional punctuation marks", () => {
    expect(polishTranscript("Erster Satz punkt neuer Absatz zweiter Satz punkt", "neutral", "light", "")).toBe("Erster Satz.\n\nZweiter Satz.");
    expect(polishTranscript("Ein Satz punkt absatz Nächster Satz punkt", "neutral", "light", "")).toBe("Ein Satz.\n\nNächster Satz.");
    expect(polishTranscript("Erster Teil semikolon zweiter Teil punkt", "neutral", "light", "")).toBe("Erster Teil; zweiter Teil.");
    expect(polishTranscript("Wort strichpunkt anderes Wort punkt", "neutral", "light", "")).toBe("Wort; anderes Wort.");
    expect(polishTranscript("Das ist wichtig gedankenstrich bitte beachten punkt", "neutral", "light", "")).toBe("Das ist wichtig — bitte beachten.");
    expect(polishTranscript("Nord bindestrich Süd punkt", "neutral", "light", "")).toBe("Nord-Süd.");
  });

  it("formats enumerations up to ten items", () => {
    expect(polishTranscript("erstens Äpfel doppelpunkt zweitens Birnen komma sechstens Pflaumen", "neutral", "light", "")).toBe("• Äpfel:\n• Birnen,\n• Pflaumen");
    expect(polishTranscript("zehntens Kirschen punkt", "neutral", "light", "")).toBe("• Kirschen.");
  });
});
