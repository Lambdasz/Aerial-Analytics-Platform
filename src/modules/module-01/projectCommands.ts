import { invoke } from "@tauri-apps/api/core";

export interface Project {
  id: string;
  name: string;
  description: string | null;
  createdAt: string;
  updatedAt: string;
}

export function createProject(name: string, description: string | null) {
  return invoke<Project>("create_project", { name, description });
}

export function getProject(projectId: string) {
  return invoke<Project>("get_project", { projectId });
}

export function listProjects() {
  return invoke<Project[]>("list_projects");
}

export function updateProject(projectId: string, name: string, description: string | null) {
  return invoke<Project>("update_project", { projectId, name, description });
}

export function deleteProject(projectId: string) {
  return invoke<void>("delete_project", { projectId });
}

export function countImagesByProject(projectId: string) {
  return invoke<number>("count_images_by_project", { projectId });
}
