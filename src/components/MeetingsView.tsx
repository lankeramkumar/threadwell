import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { formatTimestamp, messageFor } from '../lib/pure';
import type { Meeting, MeetingClaim, MeetingDetail, MeetingDoneEvent, Proposal } from '../lib/types';
import { ProposalCard } from './AssistantPanel';

const KIND_LABEL: Record<MeetingClaim['kind'], string> = {
  summary: 'Summary',
  decision: 'Decision',
  question: 'Open question',
  action: 'Action item',
};

function clock(ms: number | null): string {
  if (ms === null) return '';
  const total = Math.floor(ms / 1000);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor((total / 60) % 60))}:${pad(total % 60)}`;
}

/**
 * Transcript import and sourced extraction. Every summary line, decision, question and action
 * links to the transcript segments it came from. Action items arrive as one proposal to review.
 */
export function MeetingsView({
  onOpenPage,
  onError,
}: {
  onOpenPage: (id: string) => void;
  onError: (e: unknown) => void;
}) {
  const [meetings, setMeetings] = useState<Meeting[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [title, setTitle] = useState('');
  const [text, setText] = useState('');
  const [notice, setNotice] = useState<string | null>(null);
  const [processing, setProcessing] = useState(false);

  const loadList = useCallback(() => {
    api.meetingsList().then(setMeetings).catch(onError);
  }, [onError]);

  const loadDetail = useCallback(
    (id: string) => {
      api
        .meetingsGet(id)
        .then((d) => {
          setDetail(d);
          setProcessing(d.meeting.status === 'processing');
          if (d.meeting.proposalId) {
            api
              .aiListProposals(null)
              .then((all) => setProposals(all.filter((p) => p.id === d.meeting.proposalId)))
              .catch(() => undefined);
          } else {
            setProposals([]);
          }
        })
        .catch(onError);
    },
    [onError],
  );

  useEffect(() => {
    loadList();
  }, [loadList]);

  useEffect(() => {
    if (selected) loadDetail(selected);
    else setDetail(null);
  }, [selected, loadDetail]);

  useEffect(() => {
    const off = listen<MeetingDoneEvent>('meeting://done', (e) => {
      const done = e.payload;
      loadList();
      if (done.meetingId === selected) {
        setProcessing(false);
        if (done.status === 'failed' || done.status === 'cancelled')
          setNotice(done.message ?? 'Processing did not finish.');
        loadDetail(done.meetingId);
      }
    });
    return () => {
      off.then((u) => u());
    };
  }, [selected, loadList, loadDetail]);

  const importText = async () => {
    setNotice(null);
    try {
      const created = await api.meetingsImportText(title || 'Untitled meeting', text);
      setTitle('');
      setText('');
      loadList();
      setSelected(created.id);
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const importFile = async () => {
    setNotice(null);
    try {
      const file = await open({
        multiple: false,
        title: 'Choose a transcript',
        filters: [{ name: 'Transcripts', extensions: ['txt', 'vtt', 'srt'] }],
      });
      if (typeof file !== 'string') return;
      const created = await api.meetingsImportFile(file);
      loadList();
      setSelected(created.id);
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const importAudio = async () => {
    setNotice(null);
    try {
      const file = await open({
        multiple: false,
        title: 'Choose a recording',
        filters: [{ name: 'Audio', extensions: ['wav', 'mp3', 'm4a', 'mp4', 'webm'] }],
      });
      if (typeof file !== 'string') return;
      await api.meetingsImportAudio(file);
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const process = async () => {
    if (!selected) return;
    setNotice(null);
    setProcessing(true);
    try {
      await api.meetingsProcess(selected);
    } catch (err) {
      setProcessing(false);
      setNotice(messageFor(err));
    }
  };

  const segmentText = (ord: number) => detail?.segments.find((s) => s.ord === ord);

  return (
    <section className="meetings-view" aria-labelledby="meetings-title">
      <h1 id="meetings-title">Meetings</h1>
      <p className="muted small">
        Import a transcript. Audio needs a local transcription engine, which this build does not include.
      </p>

      {notice && (
        <p role="alert" className="inline-error">
          {notice}
        </p>
      )}

      <div className="meeting-layout">
        <aside aria-label="Imported meetings">
          <h2>Imported</h2>
          {meetings === null && <p role="status">Loading…</p>}
          {meetings?.length === 0 && <p className="muted small">None yet.</p>}
          <ul className="plain-list">
            {meetings?.map((m) => (
              <li key={m.id}>
                <button
                  type="button"
                  className={m.id === selected ? 'nav-button is-active' : 'nav-button'}
                  aria-current={m.id === selected ? 'true' : undefined}
                  onClick={() => setSelected(m.id)}
                >
                  {m.title}
                </button>
                <div className="muted small">
                  {m.segmentCount} segments · {m.status}
                </div>
              </li>
            ))}
          </ul>

          <details className="disclosure" open={!selected}>
            <summary>Import a transcript</summary>
            <div className="stack">
              <label className="field">
                <span>Title</span>
                <input value={title} maxLength={150} onChange={(e) => setTitle(e.target.value)} />
              </label>
              <label className="field">
                <span>Transcript (lines like “[00:01:02] Ana: text”, or WebVTT and SRT)</span>
                <textarea rows={6} value={text} onChange={(e) => setText(e.target.value)} />
              </label>
              <div className="row wrap">
                <button type="button" className="primary" disabled={!text.trim()} onClick={() => void importText()}>
                  Import text
                </button>
                <button type="button" onClick={() => void importFile()}>
                  Import file…
                </button>
                <button type="button" onClick={() => void importAudio()}>
                  Import audio…
                </button>
              </div>
            </div>
          </details>
        </aside>

        <div className="meeting-detail">
          {!detail && <p className="muted">Choose a meeting, or import a transcript.</p>}
          {detail && (
            <>
              <header className="view-header">
                <h2>{detail.meeting.title}</h2>
                <div className="row wrap">
                  <button type="button" onClick={() => onOpenPage(detail.meeting.pageId)}>
                    Open transcript page
                  </button>
                  <button type="button" className="primary" disabled={processing} onClick={() => void process()}>
                    {processing
                      ? 'Processing…'
                      : detail.meeting.status === 'processed'
                        ? 'Process again'
                        : 'Extract notes and actions'}
                  </button>
                </div>
              </header>
              <p role="status" className="muted small">
                Status: {detail.meeting.status}
                {detail.meeting.error ? ` · ${detail.meeting.error}` : ''} · imported{' '}
                {formatTimestamp(detail.meeting.createdAt)}
              </p>

              {detail.claims.length > 0 && (
                <section aria-label="Extracted notes">
                  <h3>Extracted</h3>
                  <ul className="claims">
                    {detail.claims.map((claim) => (
                      <li key={`${claim.kind}-${claim.ord}`} className="claim">
                        <span className="chip">{KIND_LABEL[claim.kind]}</span> {claim.text}
                        <div className="muted small">
                          Evidence:{' '}
                          {claim.segmentOrds.map((ord) => {
                            const seg = segmentText(ord);
                            return (
                              <span key={ord} title={seg ? seg.text : ''} className="evidence">
                                [{seg && seg.startMs !== null ? clock(seg.startMs) : `#${ord}`}]{' '}
                              </span>
                            );
                          })}
                        </div>
                      </li>
                    ))}
                  </ul>
                </section>
              )}

              {proposals.map((p) => (
                <ProposalCard
                  key={p.id}
                  proposal={p}
                  onChanged={() => selected && loadDetail(selected)}
                  onError={onError}
                />
              ))}

              <section aria-label="Transcript">
                <h3>Transcript</h3>
                <ol className="segments">
                  {detail.segments.map((s) => (
                    <li key={s.ord} id={`seg-${s.ord}`}>
                      <span className="muted small">{clock(s.startMs)}</span>{' '}
                      {s.speaker && <strong>{s.speaker}: </strong>}
                      {s.text}
                    </li>
                  ))}
                </ol>
              </section>
            </>
          )}
        </div>
      </div>
    </section>
  );
}
