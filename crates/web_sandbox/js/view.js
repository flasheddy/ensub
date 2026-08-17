function text(element, value) {
  element.textContent = value;
}

export function createView(document) {
  let writable = true;
  const elements = {
    form: document.querySelector("[data-testid=parse-form]"),
    input: document.querySelector("[data-testid=parse-input]"),
    candidates: document.querySelector("[data-testid=candidates]"),
    capture: document.querySelector("[data-testid=capture]"),
    stats: document.querySelector("[data-testid=stats]"),
    review: document.querySelector("[data-testid=review]"),
    reset: document.querySelector("[data-testid=reset]"),
    status: document.querySelector("[data-testid=status]"),
  };

  function report(error) {
    text(elements.status, error?.message ?? String(error));
    elements.status.dataset.errorCode = error?.code ?? "unknown";
  }

  return {
    bind(actions) {
      elements.form.addEventListener("submit", (event) => {
        event.preventDefault();
        Promise.resolve(actions.parse(elements.input.value)).catch(report);
      });
      elements.capture.addEventListener("click", () => {
        const ids = [...elements.candidates.querySelectorAll("input:checked")]
          .map((input) => input.value);
        Promise.resolve(actions.capture(ids)).catch(report);
      });
      elements.review.addEventListener("click", (event) => {
        const action = event.target.closest("button")?.dataset.action;
        if (action === "reveal") actions.reveal();
        if (action === "rate") Promise.resolve(actions.rate(Number(event.target.dataset.rating))).catch(report);
      });
      elements.reset.addEventListener("click", () => Promise.resolve(actions.reset()).catch(report));
    },

    setWritable(value) {
      writable = value;
      elements.capture.disabled = !writable;
      elements.reset.disabled = !writable;
      if (!writable) text(elements.status, "Read-only: this browser does not provide Web Locks.");
    },

    renderCandidates(output) {
      elements.candidates.replaceChildren(...(output.candidates ?? []).map((candidate) => {
        const label = document.createElement("label");
        const input = document.createElement("input");
        input.type = "checkbox";
        input.value = candidate.id;
        input.checked = true;
        label.append(input, ` ${candidate.surface} /${candidate.phonetic}/ - ${candidate.definitions[0]?.text ?? ""}`);
        return label;
      }));
      elements.capture.disabled = !writable || !(output.candidates?.length);
      text(elements.status, `${output.candidates?.length ?? 0} candidates`);
    },

    renderStats(stats) {
      text(elements.stats, `${stats.totalCards} cards | ${stats.dueCards} due`);
    },

    renderReview(card, revealed) {
      if (!card) {
        elements.review.replaceChildren(document.createTextNode("No cards due"));
        return;
      }
      const heading = document.createElement("h3");
      text(heading, card.lemma);
      const context = document.createElement("p");
      text(context, card.contexts[0]?.sentence ?? card.term);
      const controls = document.createElement("div");
      if (!revealed) {
        const reveal = document.createElement("button");
        reveal.type = "button";
        reveal.dataset.action = "reveal";
        text(reveal, "Reveal");
        controls.append(reveal);
      } else {
        const answer = document.createElement("p");
        answer.dataset.testid = "answer";
        text(answer, `/${card.phonetic}/ ${card.definition}`);
        controls.append(answer);
        for (let rating = 0; rating <= 5; rating += 1) {
          const button = document.createElement("button");
          button.type = "button";
          button.dataset.action = "rate";
          button.dataset.rating = String(rating);
          text(button, String(rating));
          controls.append(button);
        }
      }
      elements.review.replaceChildren(heading, context, controls);
    },

    report,
  };
}
