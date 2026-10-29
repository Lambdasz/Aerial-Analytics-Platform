import { useEffect, useState, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Button,
  Card,
  Elevation,
  HTMLSelect,
  InputGroup,
  NonIdealState,
  Spinner,
  Switch,
  Tag,
  Intent,
  Callout,
  Dialog,
} from "@blueprintjs/core";
import { listProjects, type Project } from "../modules/module-01/projectCommands";
import {
  getSessionsByProject,
  getImagesBySession,
  commandErrorMessage,
  type Session,
  type StoredImage,
} from "../modules/module-01/sessionCommands";
import {
  getInstalledPlugins,
  setPluginStatus,
  checkPluginHealth,
  getPluginDetails,
  formatErrorMessage,
} from "./pluginCommands";
import { PluginExecutionModal, type PluginExecutionTarget } from "./PluginExecutionModal";
import type {
  PluginSummary,
  HealthStatusDto,
  PluginDetailsDto,
  StandardJobResult,
} from "../types/plugin";
import "./PluginManager.css";

export function PluginManager() {
  const [plugins, setPlugins] = useState<PluginSummary[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [errorNotice, setErrorNotice] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [categoryFilter, setCategoryFilter] = useState<string>("ALL");
  const [statusFilter, setStatusFilter] = useState<"ALL" | "ENABLED" | "DISABLED">("ALL");

  const [selectedPluginId, setSelectedPluginId] = useState<string | null>(null);
  const [healthCheckingId, setHealthCheckingId] = useState<string | null>(null);
  const [healthResults, setHealthResults] = useState<Record<string, HealthStatusDto>>({});

  // Execution state: plugin id + image chosen for the run
  const [execPluginId, setExecPluginId] = useState<string | null>(null);
  const [execTarget, setExecTarget] = useState<PluginExecutionTarget | null>(null);
  const [lastResult, setLastResult] = useState<StandardJobResult | null>(null);
  const [publishNotice, setPublishNotice] = useState<string | null>(null);
  const [publishFailed, setPublishFailed] = useState(false);

  // Run target picker (project -> session -> image, straight from the app DB)
  const [pendingPluginId, setPendingPluginId] = useState<string | null>(null);
  const [isTargetOpen, setIsTargetOpen] = useState(false);
  const [isLoadingTargets, setIsLoadingTargets] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [images, setImages] = useState<StoredImage[]>([]);
  const [selProject, setSelProject] = useState("");
  const [selSession, setSelSession] = useState("");
  const [selImage, setSelImage] = useState("");
  const [imageFilter, setImageFilter] = useState("");

  // Details modal state
  const [detailsPlugin, setDetailsPlugin] = useState<PluginDetailsDto | null>(null);
  const [isLoadingDetails, setIsLoadingDetails] = useState<boolean>(false);
  const [isDetailsOpen, setIsDetailsOpen] = useState<boolean>(false);

  // 1. Fetch installed plugins on mount
  const loadPlugins = useCallback(async () => {
    setIsLoading(true);
    setErrorNotice(null);
    try {
      const data = await getInstalledPlugins();
      setPlugins(data);
      if (data.length > 0) {
        setSelectedPluginId((prev) => prev ?? data[0].id);
      }
    } catch (err) {
      setErrorNotice(formatErrorMessage(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadPlugins();
  }, [loadPlugins]);

  /**
   * Re-runs every plugin healthcheck once the list is loaded.
   *
   * The backend persists the last healthcheck in `plugin_state.json` and
   * never re-evaluates at startup, so a stale "unhealthy" stays on screen
   * forever until someone clicks the button. Checking on load keeps the
   * summary honest and surfaces genuinely broken plugins immediately.
   */
  useEffect(() => {
    if (plugins.length === 0) return;
    let cancelled = false;
    const ids = plugins.map((plugin) => plugin.id);

    const refresh = async () => {
      for (const id of ids) {
        if (cancelled) return;
        try {
          const health = await checkPluginHealth(id);
          if (cancelled) return;
          setHealthResults((prev) => ({ ...prev, [id]: health }));
          setPlugins((prev) =>
            prev.map((plugin) =>
              plugin.id === id
                ? {
                    ...plugin,
                    health_status: health.status === "healthy" ? "healthy" : "unhealthy",
                  }
                : plugin,
            ),
          );
        } catch (err) {
          if (cancelled) return;
          setHealthResults((prev) => ({
            ...prev,
            [id]: {
              status: "unhealthy",
              plugin_id: id,
              version: "unknown",
              error: formatErrorMessage(err),
            },
          }));
          setPlugins((prev) =>
            prev.map((plugin) =>
              plugin.id === id ? { ...plugin, health_status: "unhealthy" } : plugin,
            ),
          );
        }
      }
    };

    void refresh();
    return () => {
      cancelled = true;
    };
    // Keyed on the count: health updates replace the array identity, which
    // would restart the sweep forever.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plugins.length]);

  // 2. Optimistic Toggle with Rollback
  const handleToggle = async (pluginId: string, currentEnabled: boolean) => {
    const nextEnabled = !currentEnabled;

    // Optimistic UI update
    setPlugins((prev) => prev.map((p) => (p.id === pluginId ? { ...p, enabled: nextEnabled } : p)));

    try {
      await setPluginStatus(pluginId, nextEnabled);
    } catch (err) {
      // Rollback on error
      setPlugins((prev) =>
        prev.map((p) => (p.id === pluginId ? { ...p, enabled: currentEnabled } : p)),
      );
      setErrorNotice(`Failed to update status for plugin ${pluginId}: ${formatErrorMessage(err)}`);
    }
  };

  // 3. Health Check Trigger
  const handleHealthCheck = async (pluginId: string) => {
    setHealthCheckingId(pluginId);
    try {
      const health = await checkPluginHealth(pluginId);
      setHealthResults((prev) => ({ ...prev, [pluginId]: health }));
      setPlugins((prev) =>
        prev.map((p) =>
          p.id === pluginId
            ? { ...p, health_status: health.status === "healthy" ? "healthy" : "unhealthy" }
            : p,
        ),
      );
    } catch (err) {
      setHealthResults((prev) => ({
        ...prev,
        [pluginId]: {
          status: "unhealthy",
          plugin_id: pluginId,
          version: "unknown",
          error: formatErrorMessage(err),
        },
      }));
      setPlugins((prev) =>
        prev.map((p) => (p.id === pluginId ? { ...p, health_status: "unhealthy" } : p)),
      );
    } finally {
      setHealthCheckingId(null);
    }
  };

  // 4. View Plugin Details / Inspect schemas
  const handleInspectDetails = async (pluginId: string) => {
    setIsLoadingDetails(true);
    setIsDetailsOpen(true);
    try {
      const details = await getPluginDetails(pluginId);
      setDetailsPlugin(details);
    } catch (err) {
      setErrorNotice(formatErrorMessage(err));
      setIsDetailsOpen(false);
    } finally {
      setIsLoadingDetails(false);
    }
  };

  // 5. Run plugin: pick project -> session -> image recorded by Module 1
  const handleRunPlugin = async (pluginId: string) => {
    setErrorNotice(null);
    setPendingPluginId(pluginId);
    setSelImage("");
    setImageFilter("");
    setIsTargetOpen(true);
    if (projects.length > 0) return;
    setIsLoadingTargets(true);
    try {
      const data = await listProjects();
      setProjects(data);
      if (data.length > 0) {
        setSelProject((prev) => prev || data[0].id);
        const loaded = await getSessionsByProject(data[0].id);
        setSessions(loaded);
        if (loaded.length > 0) {
          setSelSession(loaded[0].id);
          setImages(await getImagesBySession(loaded[0].id));
        }
      }
    } catch (err) {
      setErrorNotice(commandErrorMessage(err));
    } finally {
      setIsLoadingTargets(false);
    }
  };

  const handleSelectProject = async (projectId: string) => {
    setSelProject(projectId);
    setSelSession("");
    setSessions([]);
    setImages([]);
    setSelImage("");
    setIsLoadingTargets(true);
    try {
      const loaded = await getSessionsByProject(projectId);
      setSessions(loaded);
      if (loaded.length > 0) {
        setSelSession(loaded[0].id);
        setImages(await getImagesBySession(loaded[0].id));
      }
    } catch (err) {
      setErrorNotice(commandErrorMessage(err));
    } finally {
      setIsLoadingTargets(false);
    }
  };

  const handleSelectSession = async (sessionId: string) => {
    setSelSession(sessionId);
    setImages([]);
    setSelImage("");
    setIsLoadingTargets(true);
    try {
      setImages(await getImagesBySession(sessionId));
    } catch (err) {
      setErrorNotice(commandErrorMessage(err));
    } finally {
      setIsLoadingTargets(false);
    }
  };

  const handleConfirmTarget = () => {
    const image = images.find((item) => item.id === selImage);
    if (!image || !pendingPluginId) return;
    const format = (image.format ?? "").toLowerCase();
    const extension = image.filePath.split(/[/\\]/).pop()?.split(".").pop()?.toLowerCase() ?? "";
    const mimeType =
      format === "png" || extension === "png"
        ? "image/png"
        : format === "tiff" || extension === "tif" || extension === "tiff"
          ? "image/tiff"
          : "image/jpeg";
    setLastResult(null);
    setExecPluginId(pendingPluginId);
    setExecTarget({
      imagePath: image.filePath,
      mimeType,
      sessionId: image.sessionId,
    });
    setIsTargetOpen(false);
  };

  const filteredImages = useMemo(() => {
    const query = imageFilter.trim().toLowerCase();
    if (!query) return images;
    return images.filter((image) => image.filePath.toLowerCase().includes(query));
  }, [images, imageFilter]);

  /**
   * Pushes a finished run into Module 3 storage. The backend saves the
   * GeoJSON artifact as a spatial result and emits `spatial_result:saved`,
   * which Map Explorer listens to, so the layer appears without a reload.
   */
  const publishRun = async (result: StandardJobResult, pluginName: string) => {
    setPublishNotice(null);
    setPublishFailed(false);
    try {
      const saved = await invoke<Array<{ layer_name: string }>>("publish_plugin_run", {
        result,
        layerName: `${pluginName} · ${new Date().toLocaleTimeString()}`,
      });
      const names = saved.map((layer) => layer.layer_name);
      setPublishNotice(
        saved.length === 1
          ? `Shown in Map Explorer as "${names[0]}"`
          : `Shown in Map Explorer as ${saved.length} layers: ${names.join(", ")}`,
      );
    } catch (err) {
      setPublishFailed(true);
      setPublishNotice(`Result not shown on the map: ${formatErrorMessage(err)}`);
    }
  };

  // Computed categories & filtering
  const categories = useMemo(() => {
    const set = new Set<string>();
    plugins.forEach((p) => {
      if (p.category) set.add(p.category);
    });
    return Array.from(set);
  }, [plugins]);

  const filteredPlugins = useMemo(() => {
    return plugins.filter((p) => {
      const matchesSearch =
        p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        p.id.toLowerCase().includes(searchQuery.toLowerCase()) ||
        p.description.toLowerCase().includes(searchQuery.toLowerCase());

      const matchesCategory =
        categoryFilter === "ALL" || p.category.toLowerCase() === categoryFilter.toLowerCase();

      const matchesStatus =
        statusFilter === "ALL" ||
        (statusFilter === "ENABLED" && p.enabled) ||
        (statusFilter === "DISABLED" && !p.enabled);

      return matchesSearch && matchesCategory && matchesStatus;
    });
  }, [plugins, searchQuery, categoryFilter, statusFilter]);

  const selectedPlugin = useMemo(() => {
    return plugins.find((p) => p.id === selectedPluginId) ?? plugins[0] ?? null;
  }, [plugins, selectedPluginId]);

  const enabledCount = plugins.filter((p) => p.enabled).length;

  return (
    <main className="plugin-shell">
      <section className="plugin-content">
        <header className="topbar">
          <div>
            <p className="eyebrow">Module 02 · Extension Architecture</p>
            <h1>Plugin Manager</h1>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Button
              icon="refresh"
              text="Reload Plugins"
              onClick={() => void loadPlugins()}
              loading={isLoading}
            />
          </div>
        </header>

        {errorNotice && (
          <Callout intent={Intent.DANGER} title="Notice" style={{ marginBottom: 16 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span>{errorNotice}</span>
              <Button small minimal icon="cross" onClick={() => setErrorNotice(null)} />
            </div>
          </Callout>
        )}

        <section className="stats-grid" aria-label="Plugin summary">
          <div>
            <span>Installed</span>
            <strong>{plugins.length}</strong>
          </div>
          <div>
            <span>Enabled</span>
            <strong>{enabledCount}</strong>
          </div>
          <div>
            <span>Runtime Types</span>
            <strong>
              {Array.from(new Set(plugins.map((p) => p.runtime_type))).join(", ") || "-"}
            </strong>
          </div>
          <div>
            <span>Health Status</span>
            <strong>{plugins.filter((p) => p.health_status === "healthy").length} Healthy</strong>
          </div>
        </section>

        {/* Filter controls */}
        <div
          style={{
            display: "flex",
            gap: 12,
            marginBottom: 16,
            alignItems: "center",
            flexWrap: "wrap",
          }}
        >
          <div style={{ flex: "1 1 240px" }}>
            <InputGroup
              leftIcon="search"
              placeholder="Search plugins by name, ID, or description..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              fill
            />
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <span className="bp6-text-muted" style={{ fontSize: "0.85em" }}>
              Category:
            </span>
            <Button
              small
              minimal={categoryFilter !== "ALL"}
              intent={categoryFilter === "ALL" ? Intent.PRIMARY : Intent.NONE}
              text="All"
              onClick={() => setCategoryFilter("ALL")}
            />
            {categories.map((cat) => (
              <Button
                key={cat}
                small
                minimal={categoryFilter !== cat}
                intent={categoryFilter === cat ? Intent.PRIMARY : Intent.NONE}
                text={cat}
                onClick={() => setCategoryFilter(cat)}
              />
            ))}
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <span className="bp6-text-muted" style={{ fontSize: "0.85em" }}>
              Status:
            </span>
            <Button
              small
              minimal={statusFilter !== "ALL"}
              intent={statusFilter === "ALL" ? Intent.PRIMARY : Intent.NONE}
              text="All"
              onClick={() => setStatusFilter("ALL")}
            />
            <Button
              small
              minimal={statusFilter !== "ENABLED"}
              intent={statusFilter === "ENABLED" ? Intent.PRIMARY : Intent.NONE}
              text="Enabled"
              onClick={() => setStatusFilter("ENABLED")}
            />
            <Button
              small
              minimal={statusFilter !== "DISABLED"}
              intent={statusFilter === "DISABLED" ? Intent.PRIMARY : Intent.NONE}
              text="Disabled"
              onClick={() => setStatusFilter("DISABLED")}
            />
          </div>
        </div>

        {isLoading ? (
          <div style={{ padding: "64px 0", textAlign: "center" }}>
            <Spinner size={48} />
            <p className="bp6-text-muted" style={{ marginTop: 12 }}>
              Scanning and discovering installed plugins...
            </p>
          </div>
        ) : plugins.length === 0 ? (
          <NonIdealState
            icon="box"
            title="No Plugins Installed"
            description="No analytical plugins were found in the workspace plugins directory."
            action={<Button icon="refresh" text="Scan Again" onClick={() => void loadPlugins()} />}
          />
        ) : (
          <section className="manager-grid">
            <div className="plugin-panel">
              <div className="panel-heading">
                <div>
                  <h2>Installed Plugins ({filteredPlugins.length})</h2>
                  <p>Configured analytical capabilities available to the platform</p>
                </div>
              </div>

              {filteredPlugins.length === 0 ? (
                <div style={{ padding: 32, textAlign: "center" }}>
                  <p className="bp6-text-muted">No plugins matched your filters.</p>
                </div>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {filteredPlugins.map((plugin) => {
                    const health = healthResults[plugin.id];
                    const isSelected = selectedPlugin?.id === plugin.id;
                    const isHealthy = health
                      ? health.status === "healthy"
                      : plugin.health_status === "healthy";

                    return (
                      <Card
                        key={plugin.id}
                        interactive
                        elevation={isSelected ? Elevation.TWO : Elevation.ZERO}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "space-between",
                          padding: "12px 16px",
                          borderLeft: isSelected
                            ? "4px solid var(--color-apple-blue, #0066cc)"
                            : "4px solid transparent",
                          backgroundColor: isSelected ? "rgba(19, 124, 189, 0.05)" : undefined,
                        }}
                        onClick={() => setSelectedPluginId(plugin.id)}
                      >
                        <div
                          style={{
                            display: "flex",
                            flexDirection: "column",
                            gap: 4,
                            flex: 1,
                            minWidth: 0,
                          }}
                        >
                          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                            <strong style={{ fontSize: "1.05em" }}>{plugin.name}</strong>
                            <Tag minimal round>
                              v{plugin.version}
                            </Tag>
                            <Tag minimal intent={isHealthy ? Intent.SUCCESS : Intent.WARNING}>
                              {plugin.runtime_type}
                            </Tag>
                            {plugin.category && (
                              <Tag minimal intent={Intent.PRIMARY}>
                                {plugin.category}
                              </Tag>
                            )}
                          </div>
                          <p
                            className="bp6-text-muted"
                            style={{
                              margin: 0,
                              fontSize: "0.88em",
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                              maxWidth: "90%",
                            }}
                          >
                            {plugin.description}
                          </p>
                        </div>

                        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
                          <Button
                            small
                            minimal
                            icon="pulse"
                            title="Check runtime environment health"
                            loading={healthCheckingId === plugin.id}
                            onClick={(e) => {
                              e.stopPropagation();
                              void handleHealthCheck(plugin.id);
                            }}
                          />
                          <Switch
                            checked={plugin.enabled}
                            onChange={(e) => {
                              e.stopPropagation();
                              void handleToggle(plugin.id, plugin.enabled);
                            }}
                            style={{ marginBottom: 0 }}
                          />
                        </div>
                      </Card>
                    );
                  })}
                </div>
              )}
            </div>

            {selectedPlugin && (
              <aside className="details-panel">
                <p className="eyebrow">Plugin Details</p>
                <h2>{selectedPlugin.name}</h2>
                <p className="version">
                  v{selectedPlugin.version} ·{" "}
                  <span className={selectedPlugin.enabled ? "status-ready" : "status-disabled"}>
                    {selectedPlugin.enabled ? "Ready" : "Disabled"}
                  </span>
                </p>

                <div style={{ margin: "12px 0", display: "flex", gap: 6, flexWrap: "wrap" }}>
                  <Tag intent={selectedPlugin.enabled ? Intent.SUCCESS : Intent.NONE}>
                    {selectedPlugin.enabled ? "Active" : "Disabled"}
                  </Tag>
                  <Tag intent={Intent.PRIMARY}>Runtime: {selectedPlugin.runtime_type}</Tag>
                  <Tag>Category: {selectedPlugin.category || "General"}</Tag>
                  <Tag>Author: {selectedPlugin.author || "Platform"}</Tag>
                </div>

                <p className="details-copy">{selectedPlugin.description}</p>

                {selectedPlugin.tags.length > 0 && (
                  <div style={{ marginBottom: 16 }}>
                    <span
                      className="bp6-text-muted"
                      style={{ fontSize: "0.8em", display: "block", marginBottom: 4 }}
                    >
                      TAGS
                    </span>
                    <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                      {selectedPlugin.tags.map((tag) => (
                        <Tag key={tag} minimal round>
                          {tag}
                        </Tag>
                      ))}
                    </div>
                  </div>
                )}

                {healthResults[selectedPlugin.id] && (
                  <Callout
                    intent={
                      healthResults[selectedPlugin.id].status === "healthy"
                        ? Intent.SUCCESS
                        : Intent.DANGER
                    }
                    title={`Healthcheck: ${healthResults[selectedPlugin.id].status.toUpperCase()}`}
                    style={{ marginBottom: 16 }}
                  >
                    {healthResults[selectedPlugin.id].error ||
                      "Runtime environment and dependencies verified."}
                  </Callout>
                )}

                {publishNotice && (
                  <Callout
                    intent={publishFailed ? Intent.WARNING : Intent.SUCCESS}
                    title={publishFailed ? "Map Explorer" : "Published to Map Explorer"}
                    style={{ marginBottom: 16 }}
                  >
                    {publishNotice}
                  </Callout>
                )}

                {lastResult && (
                  <Callout
                    intent={
                      lastResult.status === "failure"
                        ? Intent.DANGER
                        : lastResult.status === "warning"
                          ? Intent.WARNING
                          : Intent.SUCCESS
                    }
                    title={`Last run: ${lastResult.status.toUpperCase()} · ${lastResult.execution_time_ms} ms`}
                    style={{ marginBottom: 16 }}
                  >
                    <pre
                      style={{
                        margin: 0,
                        fontSize: "0.78em",
                        maxHeight: 160,
                        overflow: "auto",
                        whiteSpace: "pre-wrap",
                      }}
                    >
                      {JSON.stringify(
                        {
                          metrics: lastResult.metrics,
                          artifacts: lastResult.artifacts,
                          error: lastResult.error ?? null,
                        },
                        null,
                        2,
                      )}
                    </pre>
                  </Callout>
                )}

                <div style={{ display: "flex", gap: 8, marginTop: "auto" }}>
                  <Button
                    icon="play"
                    intent={Intent.SUCCESS}
                    text="Run Plugin"
                    fill
                    disabled={!selectedPlugin.enabled}
                    onClick={() => void handleRunPlugin(selectedPlugin.id)}
                  />
                  <Button
                    icon="pulse"
                    text="Run Healthcheck"
                    loading={healthCheckingId === selectedPlugin.id}
                    onClick={() => void handleHealthCheck(selectedPlugin.id)}
                  />
                </div>
                <Button
                  intent={Intent.PRIMARY}
                  text="Inspect Contract & Schemas"
                  icon="document-share"
                  onClick={() => void handleInspectDetails(selectedPlugin.id)}
                  fill
                  style={{ marginTop: 8 }}
                />
              </aside>
            )}
          </section>
        )}

        {/* Run target picker: project -> session -> image from the app database */}
        <Dialog
          isOpen={isTargetOpen}
          onClose={() => setIsTargetOpen(false)}
          title={`Run ${plugins.find((p) => p.id === pendingPluginId)?.name ?? "Plugin"}`}
          style={{ width: 560 }}
        >
          <div className="bp6-dialog-body" style={{ maxHeight: 560, overflowY: "auto" }}>
            {isLoadingTargets ? (
              <div style={{ textAlign: "center", padding: "24px 0" }}>
                <Spinner size={28} />
              </div>
            ) : (
              <>
                <div className="bp6-label">Project</div>
                <HTMLSelect
                  fill
                  value={selProject}
                  placeholder="Select a project"
                  onChange={(e) => void handleSelectProject(e.currentTarget.value)}
                  options={projects.map((project) => ({
                    label: project.name,
                    value: project.id,
                  }))}
                />

                <div className="bp6-label" style={{ marginTop: 12 }}>
                  Session
                </div>
                <HTMLSelect
                  fill
                  value={selSession}
                  placeholder="Select a session"
                  disabled={sessions.length === 0}
                  onChange={(e) => void handleSelectSession(e.currentTarget.value)}
                  options={sessions.map((session) => ({
                    label: `${session.name} (${session.dateStart?.slice(0, 10) || "no date"})`,
                    value: session.id,
                  }))}
                />

                <div className="bp6-label" style={{ marginTop: 12 }}>
                  Image ({images.length} in session)
                </div>
                <InputGroup
                  small
                  fill
                  leftIcon="search"
                  placeholder="Filter images by file name or path…"
                  value={imageFilter}
                  onChange={(e) => setImageFilter(e.target.value)}
                />
                <div
                  style={{
                    maxHeight: 240,
                    overflowY: "auto",
                    border: "1px solid var(--color-hairline-silver, #d6d6d6)",
                    borderRadius: 4,
                    marginTop: 6,
                  }}
                >
                  {filteredImages.length === 0 ? (
                    <div className="bp6-text-muted" style={{ padding: 12, fontSize: "0.85em" }}>
                      {images.length === 0
                        ? "No images in this session."
                        : "No images match the filter."}
                    </div>
                  ) : (
                    filteredImages.map((image) => (
                      <div
                        key={image.id}
                        role="button"
                        tabIndex={0}
                        onClick={() => setSelImage(image.id)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            setSelImage(image.id);
                          }
                        }}
                        style={{
                          display: "flex",
                          justifyContent: "space-between",
                          gap: 8,
                          padding: "6px 10px",
                          fontSize: "0.8em",
                          cursor: "pointer",
                          background:
                            selImage === image.id ? "rgba(19, 124, 189, 0.12)" : "transparent",
                        }}
                      >
                        <span
                          style={{
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {image.filePath.split(/[/\\]/).pop()}
                        </span>
                        <span className="bp6-text-muted">
                          {image.analysisReady === false ? "not ready" : ""}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </>
            )}
          </div>
          <div className="bp6-dialog-footer">
            <div className="bp6-dialog-footer-actions">
              <Button text="Cancel" onClick={() => setIsTargetOpen(false)} />
              <Button
                intent={Intent.SUCCESS}
                icon="play"
                text="Run"
                disabled={!selImage}
                onClick={handleConfirmTarget}
              />
            </div>
          </div>
        </Dialog>

        {/* Plugin Execution Modal */}
        {execPluginId && execTarget && (
          <PluginExecutionModal
            isOpen
            pluginId={execPluginId}
            target={execTarget}
            onClose={() => {
              setExecTarget(null);
              setExecPluginId(null);
            }}
            onSuccess={(result) => {
              const pluginName =
                plugins.find((plugin) => plugin.id === execPluginId)?.name ?? "Plugin";
              setLastResult(result);
              setExecTarget(null);
              setExecPluginId(null);
              void publishRun(result, pluginName);
            }}
          />
        )}

        {/* Contract Inspection Modal */}
        <Dialog
          isOpen={isDetailsOpen}
          onClose={() => setIsDetailsOpen(false)}
          title={`Contract Specifications: ${detailsPlugin?.manifest.metadata.name ?? ""}`}
          style={{ width: 680 }}
        >
          <div className="bp6-dialog-body" style={{ maxHeight: 500, overflowY: "auto" }}>
            {isLoadingDetails || !detailsPlugin ? (
              <div style={{ textAlign: "center", padding: "32px 0" }}>
                <Spinner size={32} />
              </div>
            ) : (
              <div>
                <h4>Parameters Specification (parameters.json)</h4>
                <pre
                  style={{
                    background: "var(--color-paper-frost, #fafafc)",
                    color: "var(--color-ink, #1d1d1f)",
                    padding: 12,
                    borderRadius: 4,
                    maxHeight: 180,
                    overflow: "auto",
                  }}
                >
                  {JSON.stringify(detailsPlugin.parameters, null, 2)}
                </pre>

                <h4>Inputs Specification (inputs.json)</h4>
                <pre
                  style={{
                    background: "var(--color-paper-frost, #fafafc)",
                    color: "var(--color-ink, #1d1d1f)",
                    padding: 12,
                    borderRadius: 4,
                    maxHeight: 140,
                    overflow: "auto",
                  }}
                >
                  {JSON.stringify(detailsPlugin.inputs, null, 2)}
                </pre>

                <h4>Outputs Specification (outputs.json)</h4>
                <pre
                  style={{
                    background: "var(--color-paper-frost, #fafafc)",
                    color: "var(--color-ink, #1d1d1f)",
                    padding: 12,
                    borderRadius: 4,
                    maxHeight: 140,
                    overflow: "auto",
                  }}
                >
                  {JSON.stringify(detailsPlugin.outputs, null, 2)}
                </pre>
              </div>
            )}
          </div>
          <div className="bp6-dialog-footer">
            <div className="bp6-dialog-footer-actions">
              <Button text="Close" onClick={() => setIsDetailsOpen(false)} />
            </div>
          </div>
        </Dialog>
      </section>
    </main>
  );
}
