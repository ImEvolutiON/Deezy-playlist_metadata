import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import {
  downloads,
  downloadHistory,
  downloadQueue,
  activeDownloads,
  pausedDownloads,
  MAX_CONCURRENT_DOWNLOADS,
  type Track,
  type QueuedDownload,
  type DownloadItem,
  type DownloadSource,
  type QualityOption
} from './stores';
import { downloadRateLimiter } from './rateLimiter';
import { notificationManager } from './notifications';

interface DownloadResult {
  file_path: string;
  requested_quality: QualityOption;
  actual_quality: QualityOption;
  status: 'complete' | 'canceled';
}

type DownloadStatus = 'downloading' | 'complete' | 'error' | 'paused' | 'resolving' | 'tagging';

const DEFAULT_PRIORITY = 0;
const HIGH_PRIORITY = 100;
const QUEUE_CHECK_INTERVAL = 1000;

class DownloadQueueManager {
  private processing = false;
  private activeCount = 0;
  private activeTrackIds = new Set<string>();
  private activeDownloadKeys = new Set<string>();
  private canceledDownloads = new Set<string>();

  async addToQueue(
    track: Track,
    source?: DownloadSource,
    priority: number = DEFAULT_PRIORITY
  ): Promise<void> {
    const trackId = String(track.id);
    const currentDownloads = get(downloads);
    const state = currentDownloads.get(trackId);
    const downloadKey = this.getDownloadKey(trackId, source);

    // Même piste + même provenance déjà active => rien à faire.
    if (this.activeDownloadKeys.has(downloadKey)) {
      console.log('Same download already active:', downloadKey);
      return;
    }

    // Pour un téléchargement direct, on conserve le comportement Deezy :
    // pas de re-téléchargement d'une piste déjà terminée ou en cours.
    //
    // Pour une playlist, on autorise le re-téléchargement :
    // le NAS fusionnera ensuite PLAYLIST / DEEZER_PLAYLIST_ID et supprimera
    // le fichier audio temporaire s'il s'agit d'un doublon.
    if (
      !source &&
      (
        this.activeTrackIds.has(trackId) ||
        ['resolving', 'downloading', 'tagging', 'complete'].includes(state ?? '')
      )
    ) {
      console.log('Track already downloading or complete:', trackId);
      return;
    }

    this.removeFromPausedSet(trackId);

    downloadQueue.update(queue => {
      if (this.isDownloadInQueue(queue, trackId, source)) {
        return queue;
      }

      return this.sortQueueByPriority([
        ...queue,
        { track, priority, source }
      ]);
    });

    if (!this.processing) {
      void this.processQueue();
    }
  }

  private getDownloadKey(trackId: string, source?: DownloadSource): string {
    return source
      ? `${trackId}:playlist:${source.playlistId}`
      : `${trackId}:direct`;
  }

  private isDownloadInQueue(
    queue: QueuedDownload[],
    trackId: string,
    source?: DownloadSource
  ): boolean {
    const key = this.getDownloadKey(trackId, source);

    return queue.some(
      item => this.getDownloadKey(String(item.track.id), item.source) === key
    );
  }

  private removeFromPausedSet(trackId: string): void {
    const paused = get(pausedDownloads);

    if (paused.has(trackId)) {
      paused.delete(trackId);
      pausedDownloads.set(paused);
    }
  }

  private sortQueueByPriority(queue: QueuedDownload[]): QueuedDownload[] {
    return queue.sort((a, b) => b.priority - a.priority);
  }

  private async processQueue(): Promise<void> {
    if (this.processing) {
      return;
    }

    this.processing = true;

    try {
      while (true) {
        const queue = get(downloadQueue);

        if (queue.length === 0) {
          break;
        }

        if (this.activeCount >= MAX_CONCURRENT_DOWNLOADS) {
          await this.waitForSlot();
          continue;
        }

        // Une même piste ne doit jamais être téléchargée deux fois en parallèle,
        // même si elle provient de deux playlists différentes.
        const itemIndex = queue.findIndex(item => {
          const trackId = String(item.track.id);

          return (
            !this.isPaused(trackId) &&
            !this.activeTrackIds.has(trackId)
          );
        });

        if (itemIndex < 0) {
          break;
        }

        const item = queue[itemIndex];

        downloadQueue.update(q =>
          q.filter((_, index) => index !== itemIndex)
        );

        void this.downloadTrack(item.track, item.source);
      }
    } finally {
      this.processing = false;
    }
  }

  private async waitForSlot(): Promise<void> {
    await new Promise(resolve =>
      setTimeout(resolve, QUEUE_CHECK_INTERVAL)
    );
  }

  private createDownloadHistoryItem(
    track: Track,
    trackId: string,
    source?: DownloadSource
  ): DownloadItem {
    return {
      trackId,
      title: track.title,
      artist: track.artist,
      album: track.album,
      cover: track.cover_medium || track.cover_small,
      percent: 0,
      status: 'downloading',
      track,
      isPaused: false,
      timestamp: new Date().toISOString(),
      source
    };
  }

  private updateDownloadStatus(
    trackId: string,
    status: DownloadStatus
  ): void {
    downloads.update(d => {
      d.set(trackId, status);
      return d;
    });
  }

  private updateHistoryItem(
    trackId: string,
    updates: Partial<DownloadItem>
  ): void {
    downloadHistory.update(history =>
      history.map(item =>
        item.trackId === trackId
          ? { ...item, ...updates }
          : item
      )
    );
  }

  private addToHistory(
    track: Track,
    trackId: string,
    source?: DownloadSource
  ): void {
    downloadHistory.update(history => {
      const existing = history.find(item => item.trackId === trackId);

      if (!existing) {
        return [
          this.createDownloadHistoryItem(track, trackId, source),
          ...history
        ];
      }

      return history.map(item =>
        item.trackId === trackId
          ? {
              ...item,
              status: 'downloading',
              percent: 0,
              isPaused: false,
              errorMsg: undefined,
              timestamp: new Date().toISOString(),
              source
            }
          : item
      );
    });
  }

  private async downloadTrack(
    track: Track,
    source?: DownloadSource
  ): Promise<void> {
    const trackId = String(track.id);

    if (this.isPaused(trackId)) {
      console.log('Track is paused, skipping:', trackId);
      return;
    }

    // On incrémente avant le try ; didIncrement évite un double décrément.
    this.incrementActiveCount(trackId, source);
    let didIncrement = true;

    this.addToHistory(track, trackId, source);
    this.updateDownloadStatus(trackId, 'downloading');

    try {
      await downloadRateLimiter.throttle();

      if (this.isPaused(trackId)) {
        console.log(
          'Track was paused during rate limiting, aborting:',
          trackId
        );

        didIncrement = false;
        this.decrementActiveCount(trackId, source);
        return;
      }

      const result = await invoke<DownloadResult>('download_track', {
        trackId,
        playlistId: source?.playlistId ?? null,
        playlistName: source?.playlistName ?? null
      });

      if (result.status === 'canceled') {
        this.canceledDownloads.add(trackId);

        this.updateHistoryItem(trackId, {
          status: 'paused',
          isPaused: true
        });

        this.updateDownloadStatus(trackId, 'paused');

        didIncrement = false;
        this.decrementActiveCount(trackId, source);
        return;
      }

      console.log('Download completed:', result.file_path);

      this.canceledDownloads.delete(trackId);
      this.removeFromPausedSet(trackId);

      this.updateDownloadStatus(trackId, 'complete');

      this.updateHistoryItem(trackId, {
        percent: 100,
        status: 'complete',
        isPaused: false,
        filePath: result.file_path,
        requestedQuality: result.requested_quality,
        actualQuality: result.actual_quality,
        source
      });

      await notificationManager.notifyDownloadComplete(
        track.title,
        track.artist
      );
    } catch (err) {
      if (this.isPaused(trackId)) {
        console.log('Download was paused:', trackId);

        didIncrement = false;
        this.decrementActiveCount(trackId, source);
        return;
      }

      console.error('Download failed:', err);

      this.updateDownloadStatus(trackId, 'error');

      this.updateHistoryItem(trackId, {
        status: 'error',
        errorMsg: String(err),
        isPaused: false,
        source
      });

      await notificationManager.notifyDownloadError(
        track.title,
        track.artist,
        String(err)
      );
    } finally {
      if (didIncrement) {
        this.decrementActiveCount(trackId, source);
      }

      // Si processQueue s'était arrêté parce que toutes les entrées restantes
      // concernaient une piste déjà active, on le relance maintenant.
      if (!this.processing && get(downloadQueue).length > 0) {
        void this.processQueue();
      }
    }
  }

  private incrementActiveCount(
    trackId: string,
    source?: DownloadSource
  ): void {
    this.activeCount++;
    this.activeTrackIds.add(trackId);
    this.activeDownloadKeys.add(
      this.getDownloadKey(trackId, source)
    );

    activeDownloads.set(this.activeCount);
  }

  private decrementActiveCount(
    trackId: string,
    source?: DownloadSource
  ): void {
    this.activeTrackIds.delete(trackId);
    this.activeDownloadKeys.delete(
      this.getDownloadKey(trackId, source)
    );

    this.activeCount = Math.max(0, this.activeCount - 1);
    activeDownloads.set(this.activeCount);
  }

  async pauseDownload(trackId: string): Promise<void> {
    const paused = get(pausedDownloads);

    paused.add(trackId);
    pausedDownloads.set(paused);

    if (this.activeTrackIds.has(trackId)) {
      try {
        await invoke<boolean>('cancel_download', { trackId });
      } catch (error) {
        console.error(
          'Failed to cancel active download:',
          error
        );
      }
    }

    const queuedItem = get(downloadQueue).find(
      item => String(item.track.id) === trackId
    );

    if (queuedItem) {
      this.addToHistory(
        queuedItem.track,
        trackId,
        queuedItem.source
      );
    }

    this.updateHistoryItem(trackId, {
      status: 'paused',
      isPaused: true
    });

    this.updateDownloadStatus(trackId, 'paused');
  }

  resumeDownload(trackId: string): void {
    if (this.activeTrackIds.has(trackId)) {
      return;
    }

    const history = get(downloadHistory);
    const item = history.find(h => h.trackId === trackId);

    const isQueuedPaused = get(downloadQueue).some(
      q => String(q.track.id) === trackId
    );

    const canResume =
      this.canceledDownloads.has(trackId) ||
      isQueuedPaused ||
      item?.status === 'paused';

    if (!canResume) {
      return;
    }

    const paused = get(pausedDownloads);

    paused.delete(trackId);
    pausedDownloads.set(paused);

    if (!item?.track) {
      return;
    }

    this.canceledDownloads.delete(trackId);

    this.updateHistoryItem(trackId, {
      status: 'downloading',
      percent: 0,
      isPaused: false,
      errorMsg: undefined
    });

    downloads.update(d => {
      d.delete(trackId);
      return d;
    });

    void this.addToQueue(
      item.track,
      item.source,
      HIGH_PRIORITY
    );
  }

  isPaused(trackId: string): boolean {
    return get(pausedDownloads).has(trackId);
  }

  clearQueue(): void {
    downloadQueue.set([]);
  }

  async prepareForExit(): Promise<void> {
    this.clearQueue();

    const activeTrackIds = this.getActiveTrackIds();

    await Promise.all(
      activeTrackIds.map(trackId =>
        this.pauseDownload(trackId)
      )
    );

    const deadline = Date.now() + 5000;

    while (
      this.activeCount > 0 &&
      Date.now() < deadline
    ) {
      await new Promise(resolve =>
        setTimeout(resolve, 50)
      );
    }
  }

  getQueueLength(): number {
    return get(downloadQueue).length;
  }

  reorderQueue(newQueue: QueuedDownload[]): void {
    downloadQueue.set(newQueue);
  }

  removeFromQueue(trackId: string): void {
    downloadQueue.update(queue =>
      queue.filter(
        item => String(item.track.id) !== trackId
      )
    );
  }

  getActiveTrackIds(): string[] {
    return Array.from(this.activeTrackIds);
  }

  getActiveCount(): number {
    return this.activeCount;
  }

  isProcessing(): boolean {
    return this.processing;
  }
}

export const downloadQueueManager = new DownloadQueueManager();
