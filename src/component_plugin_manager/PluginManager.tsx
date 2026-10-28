import { useEffect, useState, useMemo, useCallback } from "react";
import {
  Button,
  Card,
  Elevation,
  InputGroup,
  NonIdealState,
  Spinner,
  Switch,
  Tag,
  Intent,
  Callout,
  Dialog,
} from "@blueprintjs/core";
import {
  getInstalledPlugins,
  setPluginStatus,
  checkPluginHealth,
  getPluginDetails,
  formatErrorMessage,
} from "./pluginCommands";
import type { PluginSummary, HealthStatusDto, PluginDetailsDto } from "../types/plugin";
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
                          borderLeft: isSelected ? "4px solid #137cbd" : "4px solid transparent",
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

                <div style={{ display: "flex", gap: 8, marginTop: "auto" }}>
                  <Button
                    intent={Intent.PRIMARY}
                    text="Inspect Contract & Schemas"
                    icon="document-share"
                    onClick={() => void handleInspectDetails(selectedPlugin.id)}
                    fill
                  />
                  <Button
                    icon="pulse"
                    text="Run Healthcheck"
                    loading={healthCheckingId === selectedPlugin.id}
                    onClick={() => void handleHealthCheck(selectedPlugin.id)}
                  />
                </div>
              </aside>
            )}
          </section>
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
                    background: "#202b33",
                    color: "#f5f8fa",
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
                    background: "#202b33",
                    color: "#f5f8fa",
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
                    background: "#202b33",
                    color: "#f5f8fa",
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
