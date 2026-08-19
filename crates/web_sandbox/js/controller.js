const MAX_CANDIDATES = 100;

export function createController({ client, view, clock = () => Date.now() }) {
  let parsedText = "";
  let parsedCandidates = [];
  let activeReview = null;

  async function refresh() {
    const asOfMs = clock();
    const [stats, due] = await Promise.all([
      Promise.resolve(client.stats({ asOfMs })),
      Promise.resolve(client.dueReviews({ asOfMs, limit: 1 })),
    ]);
    activeReview = due.cards[0] ?? null;
    view.renderStats(stats);
    view.renderReview(activeReview, false);
  }

  async function parse(text) {
    parsedText = text;
    const output = client.parse({
      text,
      includeStopwords: false,
      maxCandidates: MAX_CANDIDATES,
    });
    parsedCandidates = output.candidates;
    view.renderCandidates(output);
  }

  async function capture(candidateIds) {
    await client.captureParsed({
      text: parsedText,
      candidateIds,
      source: "ensub-core:sandbox",
      capturedAtMs: clock(),
      includeStopwords: false,
      maxCandidates: MAX_CANDIDATES,
    });
    await refresh();
  }

  function reveal() {
    view.renderReview(activeReview, true);
  }

  async function rate(rating) {
    if (!activeReview) return;
    await client.review({
      wordId: activeReview.wordId,
      reviewToken: activeReview.reviewToken,
      rating,
      reviewedAtMs: clock(),
    });
    await refresh();
  }

  async function reset() {
    await client.reset();
    parsedText = "";
    parsedCandidates = [];
    view.renderCandidates({ candidates: [] });
    await refresh();
  }

  view.bind({ parse, capture, reveal, rate, reset });
  view.setWritable(client.writable);
  return { refresh, parse, capture, reveal, rate, reset, get parsedCandidates() { return parsedCandidates; } };
}
