import { useState } from "react";
import {
  Button,
  Callout,
  Card,
  H3,
  HTMLTable,
  InputGroup,
  Tag,
  FormGroup,
  ButtonGroup,
  Tooltip,
  Intent,
  Spinner,
  NonIdealState,
} from "@blueprintjs/core";

import {
  type Session,
  type SessionStatus,
  assignImageToSession,
  commandErrorMessage,
  createSession,
  deleteSession,
  getSessionsByProject,
  updateSessionName,
  updateSessionStatus,
} from "./sessionCommands";

/**
 * Flight Session Management page (Module 1.4).
 *
 * Clean, functional UI built with Blueprint.js components. Exercises every
 * Session command end-to-end. No Project management UI exists yet (no
 * `create_project` command), so the Project ID is entered manually; use
 * "Generate" to get a throwaway UUID for testing.
 */
export function Module01Page() {
  const [projectId, setProjectId] = useState("");
  const [sessions, setSessions] = useState<Session[]>([]);
  const [newSessionName, setNewSessionName] = useState("");
  const [imageSessionId, setImageSessionId] = useState("");
  const [imageId, setImageId] = useState("");
  const [renameDrafts, setRenameDrafts] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function withBusy(fn: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (err) {
      setError(commandErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function generateProjectId() {
    setProjectId(crypto.randomUUID());
  }

  function loadSessions() {
    void withBusy(async () => {
      const result = await getSessionsByProject(projectId);
      setSessions(result);
    });
  }

  function handleCreateSession() {
    void withBusy(async () => {
      await createSession(projectId, newSessionName.trim() || null);
      setNewSessionName("");
      setSessions(await getSessionsByProject(projectId));
    });
  }

  function handleRename(sessionId: string) {
    const draft = renameDrafts[sessionId];
    if (!draft || !draft.trim()) return;
    void withBusy(async () => {
      await updateSessionName(sessionId, draft.trim());
      setRenameDrafts((prev) => ({ ...prev, [sessionId]: "" }));
      setSessions(await getSessionsByProject(projectId));
    });
  }

  function handleToggleStatus(session: Session) {
    const next: SessionStatus = session.status === "active" ? "archived" : "active";
    void withBusy(async () => {
      await updateSessionStatus(session.id, next);
      setSessions(await getSessionsByProject(projectId));
    });
  }

  function handleDelete(sessionId: string) {
    void withBusy(async () => {
      await deleteSession(sessionId);
      setSessions(await getSessionsByProject(projectId));
    });
  }

  function handleAssignImage() {
    void withBusy(async () => {
      await assignImageToSession(imageSessionId, imageId);
      setImageId("");
      if (imageSessionId === projectId) return;
      if (sessions.some((s) => s.id === imageSessionId)) {
        setSessions(await getSessionsByProject(projectId));
      }
    });
  }

  /** Format an ISO timestamp string into a readable local date/time. */
  function formatDate(iso: string): string {
    try {
      const d = new Date(iso);
      if (isNaN(d.getTime())) return iso;
      return d.toLocaleString("id-ID", {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return iso;
    }
  }

  return (
    <section className="module-page">
      <div className="module-page__header">
        <Tag minimal>Modul 1</Tag>
        <H3 className="module-page__title">Image &amp; Project Manager</H3>
      </div>

      {error && (
        <Callout intent={Intent.DANGER} icon="error" title="Error" style={{ marginBottom: 16 }}>
          {error}
        </Callout>
      )}

      {busy && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
          <Spinner size={16} />
          <span>Memproses…</span>
        </div>
      )}

      {/* ── Project selector ─────────────────────────────────────── */}
      <Card style={{ marginBottom: 16 }}>
        <H3 style={{ marginTop: 0, fontSize: 14, marginBottom: 12 }}>Project</H3>
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            alignItems: "flex-end",
            gap: 8,
            marginBottom: 8,
          }}
        >
          <FormGroup
            label="Project ID"
            labelFor="project-id-input"
            style={{ marginBottom: 0, flex: "1 1 280px", maxWidth: "100%" }}
          >
            <InputGroup
              id="project-id-input"
              fill
              value={projectId}
              onChange={(e) => setProjectId(e.target.value)}
              placeholder="Masukkan atau generate UUID project"
              rightElement={
                <ButtonGroup minimal>
                  <Tooltip content="Generate UUID acaknya mas >///<">
                    <Button icon="random" onClick={generateProjectId} />
                  </Tooltip>
                </ButtonGroup>
              }
            />
          </FormGroup>
          <Button
            icon="refresh"
            text="Load Sessions"
            intent={Intent.PRIMARY}
            onClick={loadSessions}
            disabled={!projectId || busy}
            style={{ flexShrink: 0 }}
          />
        </div>
        <p className="bp5-text-muted" style={{ fontSize: 12, marginTop: 8, marginBottom: 0 }}>
          Project management belum tersedia ya!! ~ ~ ~ Project ID di sini hanya untuk testing ///
        </p>
      </Card>

      {/* ── Create session ───────────────────────────────────────── */}
      <Card style={{ marginBottom: 16 }}>
        <H3 style={{ marginTop: 0, fontSize: 14, marginBottom: 12 }}>Buat Session Baru</H3>
        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "flex-end", gap: 8 }}>
          <FormGroup
            label="Nama session"
            labelInfo="(opsional)"
            labelFor="new-session-name"
            style={{ marginBottom: 0, flex: "1 1 240px" }}
          >
            <InputGroup
              id="new-session-name"
              fill
              value={newSessionName}
              onChange={(e) => setNewSessionName(e.target.value)}
              placeholder="misal: Sesi pagi 1 Oktober"
              disabled={!projectId}
            />
          </FormGroup>
          <Button
            icon="plus"
            text="Create Session"
            intent={Intent.SUCCESS}
            onClick={handleCreateSession}
            disabled={!projectId || busy}
            style={{ flexShrink: 0 }}
          />
        </div>
      </Card>

      {/* ── Assign image ─────────────────────────────────────────── */}
      <Card style={{ marginBottom: 16 }}>
        <H3 style={{ marginTop: 0, fontSize: 14, marginBottom: 12 }}>
          Assign Image ke Session
          <Tag minimal intent={Intent.DANGER} style={{ marginLeft: 8, verticalAlign: "middle" }}>
            Test
          </Tag>
          <Tag minimal intent={Intent.SUCCESS} style={{ marginLeft: -12, verticalAlign: "middle" }}>
            ing
          </Tag>
          <Tag minimal intent={Intent.PRIMARY} style={{ marginLeft: -12, verticalAlign: "middle" }}>
            (〃ω〃)
          </Tag>
          <Tag minimal intent={Intent.WARNING} style={{ marginLeft: -12, verticalAlign: "middle" }}>
            He~~~~~
          </Tag>
        </H3>
        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "flex-end", gap: 8 }}>
          <FormGroup
            label="Session ID"
            labelFor="assign-session-id"
            style={{ marginBottom: 0, flex: "1 1 220px", minWidth: 160 }}
          >
            <InputGroup
              id="assign-session-id"
              fill
              value={imageSessionId}
              onChange={(e) => setImageSessionId(e.target.value)}
              placeholder="UUID session"
            />
          </FormGroup>
          <FormGroup
            label="Image ID"
            labelFor="assign-image-id"
            style={{ marginBottom: 0, flex: "1 1 220px", minWidth: 160 }}
          >
            <InputGroup
              id="assign-image-id"
              fill
              value={imageId}
              onChange={(e) => setImageId(e.target.value)}
              placeholder="UUID image"
            />
          </FormGroup>
          <Button
            icon="link"
            text="Assign"
            intent={Intent.PRIMARY}
            onClick={handleAssignImage}
            disabled={!imageSessionId || !imageId || busy}
            style={{ flexShrink: 0 }}
          />
        </div>
      </Card>

      {/* ── Sessions table ───────────────────────────────────────── */}
      <Card>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: 12,
          }}
        >
          <H3 style={{ marginTop: 0, marginBottom: 0, fontSize: 14 }}>
            Flight Sessions
            <Tag minimal round style={{ marginLeft: 8 }}>
              {sessions.length}
            </Tag>
          </H3>
        </div>

        {sessions.length === 0 ? (
          <NonIdealState
            icon="folder-open"
            title="Belum ada session"
            description='Masukkan Project ID lalu klik "Load Sessions" untuk memuat data.'
          />
        ) : (
          <div style={{ width: "100%", overflowX: "auto" }}>
            <HTMLTable
              bordered
              compact
              striped
              interactive
              style={{ width: "100%", minWidth: 600 }}
            >
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Nama</th>
                  <th>Status</th>
                  <th>Tanggal Mulai</th>
                  <th>Tanggal Selesai</th>
                  <th>Rename</th>
                  <th style={{ textAlign: "center" }}>Aksi</th>
                </tr>
              </thead>
              <tbody>
                {sessions.map((session) => (
                  <tr key={session.id}>
                    <td>
                      <Tooltip content={session.id}>
                        <code style={{ fontSize: 12 }}>{session.id.slice(0, 8)}…</code>
                      </Tooltip>
                    </td>
                    <td>{session.name}</td>
                    <td>
                      <Tag
                        minimal
                        intent={session.status === "active" ? Intent.SUCCESS : Intent.NONE}
                      >
                        {session.status}
                      </Tag>
                    </td>
                    <td style={{ fontSize: 12 }}>{formatDate(session.dateStart)}</td>
                    <td style={{ fontSize: 12 }}>{formatDate(session.dateEnd)}</td>
                    <td>
                      <div style={{ display: "flex", gap: 4 }}>
                        <InputGroup
                          small
                          value={renameDrafts[session.id] ?? ""}
                          onChange={(e) =>
                            setRenameDrafts((prev) => ({ ...prev, [session.id]: e.target.value }))
                          }
                          placeholder="Nama baru"
                          style={{ width: 140 }}
                        />
                        <Button
                          small
                          icon="edit"
                          onClick={() => handleRename(session.id)}
                          disabled={!renameDrafts[session.id]?.trim() || busy}
                        />
                      </div>
                    </td>
                    <td style={{ textAlign: "center" }}>
                      <ButtonGroup minimal>
                        <Tooltip
                          content={session.status === "active" ? "Arsipkan" : "Aktifkan kembali"}
                        >
                          <Button
                            small
                            icon={session.status === "active" ? "archive" : "unarchive"}
                            onClick={() => handleToggleStatus(session)}
                            disabled={busy}
                          />
                        </Tooltip>
                        <Tooltip content="Hapus session">
                          <Button
                            small
                            icon="trash"
                            intent={Intent.DANGER}
                            onClick={() => handleDelete(session.id)}
                            disabled={busy}
                          />
                        </Tooltip>
                      </ButtonGroup>
                    </td>
                  </tr>
                ))}
              </tbody>
            </HTMLTable>
          </div>
        )}
      </Card>
    </section>
  );
}
