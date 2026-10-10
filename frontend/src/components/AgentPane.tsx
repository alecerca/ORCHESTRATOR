import { FollowUpInput } from "./FollowUpInput";
import { MessageStream } from "./MessageStream";

interface Props {
  sessionId: string;
}

/**
 * One open agent transcript: the live stream plus the follow-up input. The
 * title, status, timer and Abort live in the group info bar above the strip.
 */
export function AgentPane({ sessionId }: Props) {
  return (
    <section className="pane">
      <MessageStream sessionId={sessionId} />
      <FollowUpInput sessionId={sessionId} />
    </section>
  );
}
