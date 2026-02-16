import { invoke } from '@tauri-apps/api/core';
import type { ClipPage, Settings, SettingsPatch } from './types';

export const listClips = async (query: string | null, limit: number, offset: number): Promise<ClipPage> =>
  invoke('list_clips', { query, limit, offset });

export const copyClip = async (id: number): Promise<void> => invoke('copy_clip', { id });

export const setPinned = async (id: number, pinned: boolean): Promise<void> => invoke('set_pinned', { id, pinned });

export const deleteClip = async (id: number): Promise<void> => invoke('delete_clip', { id });
export const clearAllClips = async (): Promise<number> => invoke('clear_all_clips');

export const getSettings = async (): Promise<Settings> => invoke('get_settings');

export const updateSettings = async (patch: SettingsPatch): Promise<Settings> => invoke('update_settings', { patch });

export const setTrackingPaused = async (paused: boolean): Promise<void> => invoke('set_tracking_paused', { paused });

export const stopApp = async (): Promise<void> => invoke('stop_app');
