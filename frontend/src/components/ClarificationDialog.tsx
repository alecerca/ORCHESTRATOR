import { useEffect, useState } from "react";
import { rejectQuestion, replyQuestion } from "../commands";
import { usePendingQuestions } from "../store";
import type { QuestionInfo } from "../types";
import { describeError } from "../utils";

/** One question's current answer: picked option labels + optional free text. */
interface AnswerState {
  labels: string[];
  text: string;
}

/**
 * Clarification pause (T11). Driven by `snapshot.questions` (the Rust core's
 * registry), so a batch is never missed even when the question event arrived
 * before the webview was listening.
 *
 * The WHOLE batch renders in ONE pause — never sequentially. Submitting sends
 * one entry per question in order (`[]` for anything left unanswered, which
 * still resumes the task), and the turn itself never ends: the agent resumes
 * the same task with these answers.
 */
export function ClarificationDialog() {
  const pending = usePendingQuestions();
  const [answers, setAnswers] = useState<Record<number, AnswerState>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const current = pending[0];

  // A new batch starts from a clean slate (the dialog unmounts between
  // batches — this only guards a batch id swap within one render cycle).
  useEffect(() => {
    setAnswers({});
    setError(null);
  }, [current?.id]);

  if (!current) return null;

  const questions = current.questions;

  function answerOf(index: number): AnswerState {
    return answers[index] ?? { labels: [], text: "" };
  }

  function toggleOption(index: number, question: QuestionInfo, label: string): void {
    const previous = answerOf(index);
    if (question.multiple) {
      const labels = previous.labels.includes(label)
        ? previous.labels.filter((value) => value !== label)
        : [...previous.labels, label];
      setAnswers({ ...answers, [index]: { ...previous, labels } });
    } else {
      setAnswers({
        ...answers,
        [index]: { labels: previous.labels.includes(label) ? [] : [label], text: previous.text },
      });
    }
  }

  function setText(index: number, text: string): void {
    setAnswers({ ...answers, [index]: { ...answerOf(index), text } });
  }

  /** The reply entry for one question: labels, plus free text where allowed. */
  function entryOf(question: QuestionInfo, index: number): string[] {
    const { labels, text } = answerOf(index);
    const trimmed = text.trim();
    if (trimmed === "") return [...labels];
    if (question.multiple) return [...labels, trimmed];
    // Single-select: an explicit free-text answer wins over an option click.
    return labels.length === 0 ? [trimmed] : labels;
  }

  const answered = questions.some((_, index) => {
    const { labels, text } = answerOf(index);
    return labels.length > 0 || text.trim() !== "";
  });

  async function submit(): Promise<void> {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await replyQuestion(
        current.id,
        questions.map((question, index) => entryOf(question, index)),
      );
      setAnswers({});
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  async function reject(): Promise<void> {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await rejectQuestion(current.id);
      setAnswers({});
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }

  const first = questions[0];
  const title =
    first?.header?.trim() || first?.question?.trim() || `${questions.length} question(s)`;

  return (
    <div className="modal-backdrop">
      <div
        className="modal clarify"
        role="dialog"
        aria-modal="true"
        aria-labelledby="clarify-title"
      >
        <div className="approval__head">
          <span className="chip chip--waiting_approval">clarification</span>
          <span className="approval__type">{questions.length} question{questions.length > 1 ? "s" : ""}</span>
          {pending.length > 1 && (
            <span className="approval__queue">+{pending.length - 1} queued</span>
          )}
        </div>
        <h2 className="approval__title" id="clarify-title">
          {title}
        </h2>

        <div className="clarify__list">
          {questions.map((question, index) => {
            const currentAnswer = answerOf(index);
            const freeTextAllowed = question.custom !== false;
            return (
              <section key={index} className="clarify__question">
                <p className="clarify__text">{question.question}</p>
                {question.options.length > 0 && (
                  <div
                    className="clarify__options"
                    role={question.multiple ? "group" : "radiogroup"}
                    aria-label={question.header || question.question}
                  >
                    {question.options.map((option) => {
                      const selected = currentAnswer.labels.includes(option.label);
                      return (
                        <button
                          key={option.label}
                          type="button"
                          className={`clarify__option${selected ? " is-selected" : ""}`}
                          aria-pressed={selected}
                          disabled={busy}
                          onClick={() => toggleOption(index, question, option.label)}
                        >
                          <span className="clarify__option-label">
                            {question.recommended === option.label && (
                              <span className="oc-free" title="Recommended">
                                recommended
                              </span>
                            )}
                            {option.label}
                          </span>
                          {option.description && (
                            <span className="clarify__option-desc">{option.description}</span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
                {freeTextAllowed && (
                  <label className="clarify__free">
                    <span>{question.options.length > 0 ? "…or type your own answer" : "Your answer"}</span>
                    <input
                      className="settings-input"
                      value={currentAnswer.text}
                      placeholder="Type your answer…"
                      disabled={busy}
                      onChange={(event) => setText(index, event.target.value)}
                    />
                  </label>
                )}
              </section>
            );
          })}
        </div>

        <p className="clarify__hint">
          Unanswered questions are skipped — the task resumes with what you provide.
        </p>
        {error && (
          <p className="approval__error" role="alert">
            {error}
          </p>
        )}
        <div className="approval__actions">
          <button
            type="button"
            className="btn btn--primary"
            disabled={busy || !answered}
            autoFocus
            onClick={() => void submit()}
          >
            {busy ? "Sending…" : "Answer"}
          </button>
          <button
            type="button"
            className="btn btn--danger"
            disabled={busy}
            onClick={() => void reject()}
          >
            Reject
          </button>
        </div>
      </div>
    </div>
  );
}
