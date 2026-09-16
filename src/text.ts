export type Tone = "neutral" | "professional" | "casual";
export type ProcessingLevel = "light" | "standard" | "strong";

const fillerWords = ["äh", "ähm", "eh", "hm", "mhm", "äh also", "also äh", "sozusagen", "quasi", "im prinzip", "eigentlich", "halt", "you know", "um", "uh"];
const spokenPunctuation: Array<[RegExp, string]> = [
  [/\s+komma\s*/gi, ", "], [/\s+punkt\s*/gi, ". "], [/\s+doppelpunkt\s*/gi, ": "],
  [/\s+ausrufezeichen\s*/gi, "! "], [/\s+fragezeichen\s*/gi, "? "], [/\s+neue zeile\s*/gi, "\n"],
];

/** Applies only deterministic, on-device transformations; it never calls a model or service. */
export function polishTranscript(input: string, tone: Tone, level: ProcessingLevel, dictionary: string) {
  let text = input.normalize("NFC").replace(/\r\n?/g, "\n").replace(/^\s*(?:\[(?:MUSIK|APPLAUSE|UNVERSTÄNDLICH|BLANK_AUDIO)\]|\((?:Musik|Applaus)\))\s*/giu, "");
  for (const [pattern, replacement] of spokenPunctuation) text = text.replace(pattern, replacement);
  if (level !== "light") text = resolveTimeCorrections(text);
  if (level !== "light") text = removeFillers(text, level);
  text = removeDirectRepetitions(text);
  if (level === "strong") text = removeSimpleFalseStarts(text);
  text = applyDictionary(text, dictionary);
  text = formatEnumerations(text);
  text = normalizeSpacing(text);
  text = finishSentences(text);
  return applyTone(text, tone);
}

function resolveTimeCorrections(text: string) {
  // Only rewrite an unambiguous correction when an earlier "um <time> Uhr" exists.
  // This preserves all surrounding wording and avoids guessing at arbitrary corrections.
  const correction = /(?:[.!?]+\s*)?(?:oh,?\s*)?(?:ich\s+meine|korrektur[:\s]+)\s+(\d{1,2}(?::\d{2})?\s*Uhr)\b/giu;
  let match: RegExpExecArray | null;
  while ((match = correction.exec(text))) {
    const prefix = text.slice(0, match.index);
    const previous = [...prefix.matchAll(/\bum\s+\d{1,2}(?::\d{2})?\s*Uhr\b/giu)].pop();
    if (!previous || previous.index === undefined) continue;
    const corrected = `um ${match[1]}`;
    text = `${prefix.slice(0, previous.index)}${corrected}${prefix.slice(previous.index + previous[0].length)}${text.slice(match.index + match[0].length)}`;
    correction.lastIndex = previous.index + corrected.length;
  }
  return text;
}

function removeFillers(text: string, level: ProcessingLevel) {
  const active = level === "strong" ? fillerWords : fillerWords.filter((word) => !["also", "im prinzip"].includes(word));
  const pattern = new RegExp(`(^|[\\s,])(?:${active.map(escapeRegExp).join("|")})(?=[\\s,.!?]|$)[,.]?`, "giu");
  return text.replace(pattern, "$1");
}

function removeDirectRepetitions(text: string) {
  return text.replace(/\b([\p{L}][\p{L}'-]*)\s+\1\b/giu, "$1");
}

// Handles only the unambiguous spoken pattern "… nein, <replacement>" without guessing intent.
function removeSimpleFalseStarts(text: string) {
  return text.replace(/\b(?:nein|korrektur),\s+/giu, "");
}

function applyDictionary(text: string, dictionary: string) {
  return dictionary.split("\n").reduce((result, line) => {
    const [spoken, written, ...extra] = line.split("=>").map((part) => part.trim());
    if (!spoken || !written || extra.length) return result;
    return result.replace(new RegExp(`\\b${escapeRegExp(spoken)}\\b`, "giu"), written);
  }, text);
}

function formatEnumerations(text: string) {
  return text.replace(/\b(?:erstens|zweitens|drittens|viertens|fünftens)\b\s*[:,]?\s*/giu, "\n• ");
}

function normalizeSpacing(text: string) {
  return text.replace(/[ \t]+\n/g, "\n").replace(/\n[ \t]+/g, "\n").replace(/[ \t]+([,.;:!?])/g, "$1").replace(/([,.;:!?])(?=[^\s\n])/g, "$1 ").replace(/[ \t]{2,}/g, " ").replace(/\n{3,}/g, "\n\n").trim();
}

function finishSentences(text: string) {
  const sentences = text.split(/(?<=[.!?])\s+|\n/).map((part) => {
    const value = part.trim(); if (!value) return "";
    const capped = value.replace(/^([a-zäöü])/u, (letter) => letter.toUpperCase());
    return /[.!?:;]$/.test(capped) || capped.startsWith("•") ? capped : `${capped}.`;
  });
  return sentences.filter(Boolean).join("\n");
}

function applyTone(text: string, tone: Tone) {
  if (tone === "professional") return text.replace(/\bhi\b/giu, "Guten Tag").replace(/\bdanke dir\b/giu, "Vielen Dank");
  if (tone === "casual") return text.replace(/\bguten tag\b/giu, "Hallo").replace(/\bvielen dank\b/giu, "Danke");
  return text;
}

function escapeRegExp(value: string) { return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
