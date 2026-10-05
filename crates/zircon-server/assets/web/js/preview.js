// Zircon World Seed Preview & Interactive Slippy Tile Map Studio module
window.Zircon = window.Zircon || {};

window.Zircon.preview = {
    // -------------------------------------------------------------------------
    // Slippy Canvas Tile Engine (Google Maps Experience)
    // -------------------------------------------------------------------------
    initWorldMapEngine() {
        const canvas = document.getElementById('worldMapCanvas');
        if (!canvas) return;

        if (!this.mapTileCache) this.mapTileCache = new Map();
        if (!this.mapPendingTiles) this.mapPendingTiles = new Set();
        if (!this.mapGeneratingTiles) this.mapGeneratingTiles = new Set();
        if (!this.activePaintedChunks) this.activePaintedChunks = new Set();
        if (!this.loadingChunks) this.loadingChunks = new Map();
        this.mapVelocityX = 0;
        this.mapVelocityZ = 0;
        this.mapTool = this.mapTool || 'pan';
        this.brushSize = this.brushSize || 1;
        this.isPaintingStroke = false;
        this.isSpacePressed = false;

        // Clean up previous event listeners if already attached
        if (this._mapCleanup) {
            this._mapCleanup();
        }

        let isMouseDown = false;
        let startClientX = 0;
        let startClientY = 0;

        const onWheel = (e) => {
            e.preventDefault();
            const rect = canvas.getBoundingClientRect();
            const mouseX = e.clientX - rect.left;
            const mouseY = e.clientY - rect.top;

            const zoom = this.previewZoom || 1.0;
            const cursorWorldX = (this.previewCenterX || 0) + (mouseX - rect.width / 2) / zoom;
            const cursorWorldZ = (this.previewCenterZ || 0) + (mouseY - rect.height / 2) / zoom;

            const factor = e.deltaY < 0 ? 1.25 : 0.8;
            const newZoom = Math.min(Math.max(zoom * factor, 0.15), 6.0);

            // Anchor zoom directly at mouse cursor coordinate
            this.previewCenterX = Math.round(cursorWorldX - (mouseX - rect.width / 2) / newZoom);
            this.previewCenterZ = Math.round(cursorWorldZ - (mouseY - rect.height / 2) / newZoom);
            this.previewZoom = parseFloat(newZoom.toFixed(2));
        };

        const onKeyDown = (e) => {
            if (e.code === 'Space' && !['INPUT', 'TEXTAREA'].includes(e.target.tagName)) {
                this.isSpacePressed = true;
                if (!this.mapIsDragging && !this.isPaintingStroke) {
                    canvas.style.cursor = 'grab';
                }
            }
        };

        const onKeyUp = (e) => {
            if (e.code === 'Space') {
                this.isSpacePressed = false;
                if (!this.mapIsDragging && !this.isPaintingStroke) {
                    canvas.style.cursor = (this.mapTool === 'brush') ? 'crosshair' : 'grab';
                }
            }
        };

        const onContextMenu = (e) => {
            e.preventDefault();
        };

        const onMouseDown = (e) => {
            const isPanButton = (e.button === 1 || e.button === 2 || (e.button === 0 && (this.mapTool === 'pan' || this.isSpacePressed)));

            if (isPanButton) {
                isMouseDown = true;
                this.mapIsDragging = true;
                this.isPaintingStroke = false;
                startClientX = e.clientX;
                startClientY = e.clientY;
                this.mapLastX = e.clientX;
                this.mapLastY = e.clientY;
                this.mapVelocityX = 0;
                this.mapVelocityZ = 0;
                canvas.style.cursor = 'grabbing';
                return;
            }

            if (e.button === 0 && this.mapTool === 'brush' && !this.isSpacePressed) {
                isMouseDown = true;
                this.mapIsDragging = false;
                this.isPaintingStroke = true;

                const rect = canvas.getBoundingClientRect();
                const mouseX = e.clientX - rect.left;
                const mouseY = e.clientY - rect.top;
                const zoom = this.previewZoom || 1.0;

                const curWorldX = (this.previewCenterX || 0) + (mouseX - rect.width / 2) / zoom;
                const curWorldZ = (this.previewCenterZ || 0) + (mouseY - rect.height / 2) / zoom;

                if (!this.activePaintedChunks) this.activePaintedChunks = new Set();
                this.activePaintedChunks.clear();
                this.lastPaintWorldX = curWorldX;
                this.lastPaintWorldZ = curWorldZ;
                this.addChunksAt(curWorldX, curWorldZ, this.brushSize || 1);
                canvas.style.cursor = 'crosshair';
            }
        };

        const onMouseMove = (e) => {
            const rect = canvas.getBoundingClientRect();
            const mouseX = e.clientX - rect.left;
            const mouseY = e.clientY - rect.top;
            const zoom = this.previewZoom || 1.0;

            // Update live block coordinates under cursor
            this.previewHoverX = Math.round((this.previewCenterX || 0) + (mouseX - rect.width / 2) / zoom);
            this.previewHoverZ = Math.round((this.previewCenterZ || 0) + (mouseY - rect.height / 2) / zoom);

            const hoverRx = Math.floor((this.previewHoverX || 0) / 512);
            const hoverRz = Math.floor((this.previewHoverZ || 0) / 512);
            this.previewHoverRx = hoverRx;
            this.previewHoverRz = hoverRz;

            if (this.mapIsDragging) {
                const dx = e.clientX - this.mapLastX;
                const dy = e.clientY - this.mapLastY;

                this.previewCenterX = Math.round((this.previewCenterX || 0) - dx / zoom);
                this.previewCenterZ = Math.round((this.previewCenterZ || 0) - dy / zoom);

                this.mapVelocityX = dx;
                this.mapVelocityZ = dy;

                this.mapLastX = e.clientX;
                this.mapLastY = e.clientY;
                canvas.style.cursor = 'grabbing';
            } else if (this.isPaintingStroke && this.activePaintedChunks) {
                // Smooth interpolation to paint all chunks between mouse events without gaps
                const curX = this.previewHoverX;
                const curZ = this.previewHoverZ;
                const lastX = (this.lastPaintWorldX !== undefined) ? this.lastPaintWorldX : curX;
                const lastZ = (this.lastPaintWorldZ !== undefined) ? this.lastPaintWorldZ : curZ;
                const dist = Math.hypot(curX - lastX, curZ - lastZ);
                const steps = Math.max(1, Math.ceil(dist / 8));
                for (let i = 1; i <= steps; i++) {
                    const t = i / steps;
                    const ix = lastX + (curX - lastX) * t;
                    const iz = lastZ + (curZ - lastZ) * t;
                    this.addChunksAt(ix, iz, this.brushSize || 1);
                }
                this.lastPaintWorldX = curX;
                this.lastPaintWorldZ = curZ;
                canvas.style.cursor = 'crosshair';
            } else {
                if (this.mapTool === 'brush') {
                    canvas.style.cursor = this.isSpacePressed ? 'grab' : 'crosshair';
                } else {
                    canvas.style.cursor = 'grab';
                }
            }
        };

        const onMouseUp = (e) => {
            if (!isMouseDown && !this.mapIsDragging && !this.isPaintingStroke) return;
            isMouseDown = false;

            if (this.isPaintingStroke) {
                this.isPaintingStroke = false;
                if (this.activePaintedChunks && this.activePaintedChunks.size > 0) {
                    const chunkList = [];
                    if (!this.loadingChunks) this.loadingChunks = new Map();
                    const now = Date.now();
                    for (const key of this.activePaintedChunks) {
                        const [cx, cz] = key.split(',').map(Number);
                        chunkList.push({ cx, cz });
                        this.loadingChunks.set(key, { cx, cz, startTime: now });
                    }
                    this.activePaintedChunks.clear();
                    this.paintBatchChunks(chunkList);
                }
            }

            this.mapIsDragging = false;
            canvas.style.cursor = (this.mapTool === 'brush') ? (this.isSpacePressed ? 'grab' : 'crosshair') : 'grab';
        };

        const onDblClick = (e) => {
            const rect = canvas.getBoundingClientRect();
            const mouseX = e.clientX - rect.left;
            const mouseY = e.clientY - rect.top;
            const zoom = this.previewZoom || 1.0;

            const targetWorldX = (this.previewCenterX || 0) + (mouseX - rect.width / 2) / zoom;
            const targetWorldZ = (this.previewCenterZ || 0) + (mouseY - rect.height / 2) / zoom;

            const newZoom = Math.min(zoom * 1.5, 6.0);
            this.previewCenterX = Math.round(targetWorldX);
            this.previewCenterZ = Math.round(targetWorldZ);
            this.previewZoom = parseFloat(newZoom.toFixed(2));
        };

        canvas.addEventListener('wheel', onWheel, { passive: false });
        canvas.addEventListener('mousedown', onMouseDown);
        canvas.addEventListener('contextmenu', onContextMenu);
        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
        window.addEventListener('keydown', onKeyDown);
        window.addEventListener('keyup', onKeyUp);
        canvas.addEventListener('dblclick', onDblClick);

        this._mapCleanup = () => {
            canvas.removeEventListener('wheel', onWheel);
            canvas.removeEventListener('mousedown', onMouseDown);
            canvas.removeEventListener('contextmenu', onContextMenu);
            window.removeEventListener('mousemove', onMouseMove);
            window.removeEventListener('mouseup', onMouseUp);
            window.removeEventListener('keydown', onKeyDown);
            window.removeEventListener('keyup', onKeyUp);
            canvas.removeEventListener('dblclick', onDblClick);
            if (this._mapRafId) cancelAnimationFrame(this._mapRafId);
            this._mapRafId = null;
            if (this._streamDebounceTimer) clearTimeout(this._streamDebounceTimer);
        };

        // Start render loop
        const loop = () => {
            if (this.activeTab === 'world') {
                this.renderWorldMapFrame(canvas);
                this._mapRafId = requestAnimationFrame(loop);
            }
        };
        if (this._mapRafId) cancelAnimationFrame(this._mapRafId);
        this._mapRafId = requestAnimationFrame(loop);

        // Initialized in standby mode (user clicks 'Render Map' to start worker)
        this.addPreviewLog("Interactive 60 FPS tile engine initialized in standby mode.");
    },

    renderWorldMapFrame(canvas) {
        if (!canvas) return;
        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return;

        // Retina display scaling
        const dpr = window.devicePixelRatio || 1;
        const targetW = Math.round(rect.width * dpr);
        const targetH = Math.round(rect.height * dpr);

        if (canvas.width !== targetW || canvas.height !== targetH) {
            canvas.width = targetW;
            canvas.height = targetH;
        }

        const ctx = canvas.getContext('2d');
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

        const width = rect.width;
        const height = rect.height;

        // Apply momentum inertia if not dragging
        if (!this.mapIsDragging && (Math.abs(this.mapVelocityX || 0) > 0.05 || Math.abs(this.mapVelocityZ || 0) > 0.05)) {
            const zoom = this.previewZoom || 1.0;
            this.previewCenterX = Math.round((this.previewCenterX || 0) - this.mapVelocityX / zoom);
            this.previewCenterZ = Math.round((this.previewCenterZ || 0) - this.mapVelocityZ / zoom);
            this.mapVelocityX *= 0.91;
            this.mapVelocityZ *= 0.91;
        }

        const centerX = this.previewCenterX || 0;
        const centerZ = this.previewCenterZ || 0;
        const zoom = this.previewZoom || 1.0;

        // 1. Clear background to dark void
        ctx.fillStyle = '#0c1017';
        ctx.fillRect(0, 0, width, height);

        // 2. Visible world bounds
        const halfW = width / (2 * zoom);
        const halfH = height / (2 * zoom);
        const minWorldX = centerX - halfW;
        const maxWorldX = centerX + halfW;
        const minWorldZ = centerZ - halfH;
        const maxWorldZ = centerZ + halfH;

        // Region coordinates (each region is 512x512 blocks)
        const minRx = Math.floor(minWorldX / 512);
        const maxRx = Math.floor(maxWorldX / 512);
        const minRz = Math.floor(minWorldZ / 512);
        const maxRz = Math.floor(maxWorldZ / 512);

        // 3. Render region tiles
        const instId = this.selectedInstance ? this.selectedInstance.id : 'default';
        for (let rx = minRx; rx <= maxRx; rx++) {
            for (let rz = minRz; rz <= maxRz; rz++) {
                const tileKey = `${instId}:${rx},${rz}`;
                const tileWorldX = rx * 512;
                const tileWorldZ = rz * 512;

                const screenX = width / 2 + (tileWorldX - centerX) * zoom;
                const screenY = height / 2 + (tileWorldZ - centerZ) * zoom;
                const tileSize = 512 * zoom;

                const cached = this.mapTileCache.get(tileKey);
                const isPending = this.mapPendingTiles && this.mapPendingTiles.has(tileKey);
                const isGenerating = this.mapGeneratingTiles && this.mapGeneratingTiles.has(tileKey);
                const isRendering = isPending || isGenerating;
                const hasValidImage = cached && cached !== 'none';

                if (hasValidImage) {
                    // Draw cached region tile (committed to disk)
                    ctx.imageSmoothingEnabled = zoom < 1.0;
                    ctx.drawImage(cached, screenX, screenY, tileSize, tileSize);

                    // If currently re-generating this region in the background, draw a subtle cyan boundary indicator
                    if (isGenerating) {
                        ctx.strokeStyle = 'rgba(6, 182, 212, 0.45)';
                        ctx.lineWidth = 1.5;
                        ctx.strokeRect(screenX, screenY, tileSize, tileSize);
                    }

                    // Region Boundary Overlay for committed regions
                    if (this.previewShowGrid) {
                        ctx.strokeStyle = 'rgba(56, 189, 248, 0.35)';
                        ctx.lineWidth = 1.5;
                        ctx.strokeRect(screenX, screenY, tileSize, tileSize);

                        if (tileSize > 40) {
                            ctx.fillStyle = 'rgba(56, 189, 248, 0.85)';
                            ctx.font = '10px monospace';
                            ctx.fillText(`r.${rx}.${rz}`, screenX + 6, screenY + 14);
                        }
                    }
                } else {
                    // Tile image not yet in cache
                    if (cached === undefined && !isPending) {
                        // Request tile from server
                        this.fetchRegionTile(rx, rz);
                    }

                    if (isGenerating) {
                        // Actively rendering / generating: draw dark cyan square with animated radar spinner
                        ctx.save();
                        // Dark cyan surface
                        ctx.fillStyle = 'rgba(6, 36, 46, 0.88)';
                        ctx.fillRect(screenX, screenY, tileSize, tileSize);

                        // Subtle cyber-cyan grid lines
                        ctx.strokeStyle = 'rgba(6, 182, 212, 0.16)';
                        ctx.lineWidth = 1;
                        const step = Math.max(32 * zoom, 24);
                        for (let x = 0; x < tileSize; x += step) {
                            ctx.beginPath();
                            ctx.moveTo(screenX + x, screenY);
                            ctx.lineTo(screenX + x, screenY + tileSize);
                            ctx.stroke();
                        }
                        for (let y = 0; y < tileSize; y += step) {
                            ctx.beginPath();
                            ctx.moveTo(screenX, screenY + y);
                            ctx.lineTo(screenX + tileSize, screenY + y);
                            ctx.stroke();
                        }

                        // Glowing cyan border
                        ctx.strokeStyle = 'rgba(34, 211, 238, 0.65)';
                        ctx.lineWidth = 1.5;
                        ctx.strokeRect(screenX, screenY, tileSize, tileSize);

                        // Animated radar spinner in center
                        const tileCenterX = screenX + tileSize / 2;
                        const tileCenterY = screenY + tileSize / 2;

                        if (tileSize > 40) {
                            const spinnerRadius = Math.min(Math.max(tileSize * 0.1, 14), 28);
                            const angle = (performance.now() * 0.005) % (Math.PI * 2);

                            ctx.save();
                            ctx.translate(tileCenterX, tileCenterY);

                            // Outer faint ring
                            ctx.beginPath();
                            ctx.arc(0, 0, spinnerRadius, 0, Math.PI * 2);
                            ctx.strokeStyle = 'rgba(6, 182, 212, 0.25)';
                            ctx.lineWidth = 2.5;
                            ctx.stroke();

                            // Rotating cyan arc
                            ctx.beginPath();
                            ctx.arc(0, 0, spinnerRadius, angle, angle + Math.PI * 1.2);
                            ctx.strokeStyle = '#22d3ee';
                            ctx.lineWidth = 2.5;
                            ctx.lineCap = 'round';
                            ctx.stroke();

                            // Pulsing core dot
                            const pulse = 0.5 + 0.5 * Math.sin(performance.now() * 0.008);
                            ctx.beginPath();
                            ctx.arc(0, 0, 3 + pulse * 1.5, 0, Math.PI * 2);
                            ctx.fillStyle = '#38bdf8';
                            ctx.fill();

                            ctx.restore();

                            if (tileSize > 70) {
                                ctx.fillStyle = '#67e8f9';
                                ctx.font = 'bold 10px monospace';
                                ctx.textAlign = 'center';
                                ctx.fillText('RENDERING...', tileCenterX, tileCenterY + spinnerRadius + 16);
                                ctx.font = '8px monospace';
                                ctx.fillStyle = 'rgba(148, 163, 184, 0.7)';
                                ctx.fillText(`r.${rx}.${rz}`, tileCenterX, tileCenterY + spinnerRadius + 28);
                                ctx.textAlign = 'left';
                            }
                        }
                        ctx.restore();
                    } else {
                        // Unexplored fallback blueprint grid with clean region boundary
                        ctx.save();
                        const isHovered = (this.previewHoverRx === rx && this.previewHoverRz === rz);

                        ctx.fillStyle = isHovered ? 'rgba(15, 26, 38, 0.85)' : 'rgba(10, 16, 24, 0.72)';
                        ctx.fillRect(screenX, screenY, tileSize, tileSize);

                        // Subtle hatched pattern
                        ctx.strokeStyle = isHovered ? 'rgba(56, 189, 248, 0.12)' : 'rgba(56, 189, 248, 0.05)';
                        ctx.lineWidth = 1;
                        const step = Math.max(32 * zoom, 24);
                        for (let x = 0; x < tileSize; x += step) {
                            ctx.beginPath();
                            ctx.moveTo(screenX + x, screenY);
                            ctx.lineTo(screenX + x, screenY + tileSize);
                            ctx.stroke();
                        }
                        for (let y = 0; y < tileSize; y += step) {
                            ctx.beginPath();
                            ctx.moveTo(screenX, screenY + y);
                            ctx.lineTo(screenX + tileSize, screenY + y);
                            ctx.stroke();
                        }

                        // Subtle boundary line for unexplored region
                        ctx.strokeStyle = isHovered ? 'rgba(34, 211, 238, 0.45)' : 'rgba(100, 116, 139, 0.2)';
                        ctx.lineWidth = isHovered ? 1.5 : 1;
                        ctx.strokeRect(screenX, screenY, tileSize, tileSize);

                        // Clean region ID in top-left corner
                        if (tileSize > 40) {
                            ctx.fillStyle = isHovered ? '#38bdf8' : 'rgba(148, 163, 184, 0.5)';
                            ctx.font = '10px monospace';
                            ctx.fillText(`r.${rx}.${rz}`, screenX + 8, screenY + 16);
                        }
                        ctx.restore();
                    }
                }
            }
        }

        // 4. Subtle Chunk gridlines (every 16 blocks)
        if (this.previewShowGrid && zoom >= 0.7) {
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.04)';
            ctx.lineWidth = 1;

            const startChunkX = Math.floor(minWorldX / 16) * 16;
            const endChunkX = Math.ceil(maxWorldX / 16) * 16;
            for (let cx = startChunkX; cx <= endChunkX; cx += 16) {
                const sx = Math.round(width / 2 + (cx - centerX) * zoom) + 0.5;
                ctx.beginPath();
                ctx.moveTo(sx, 0);
                ctx.lineTo(sx, height);
                ctx.stroke();
            }

            const startChunkZ = Math.floor(minWorldZ / 16) * 16;
            const endChunkZ = Math.ceil(maxWorldZ / 16) * 16;
            for (let cz = startChunkZ; cz <= endChunkZ; cz += 16) {
                const sy = Math.round(height / 2 + (cz - centerZ) * zoom) + 0.5;
                ctx.beginPath();
                ctx.moveTo(0, sy);
                ctx.lineTo(width, sy);
                ctx.stroke();
            }
        }

        // 5. World Origin Spawn Marker (0, 0)
        const spawnScreenX = width / 2 + (0 - centerX) * zoom;
        const spawnScreenY = height / 2 + (0 - centerZ) * zoom;

        if (spawnScreenX >= -40 && spawnScreenX <= width + 40 && spawnScreenY >= -40 && spawnScreenY <= height + 40) {
            ctx.save();
            ctx.strokeStyle = '#ef4444';
            ctx.lineWidth = 2;

            // Target crosshair
            ctx.beginPath();
            ctx.arc(spawnScreenX, spawnScreenY, 6, 0, Math.PI * 2);
            ctx.stroke();

            ctx.beginPath();
            ctx.moveTo(spawnScreenX - 12, spawnScreenY);
            ctx.lineTo(spawnScreenX + 12, spawnScreenY);
            ctx.moveTo(spawnScreenX, spawnScreenY - 12);
            ctx.lineTo(spawnScreenX + 12, spawnScreenY);
            ctx.stroke();

            // Label
            ctx.fillStyle = '#ef4444';
            ctx.font = 'bold 11px sans-serif';
            ctx.fillText('Spawn (0, 0)', spawnScreenX + 10, spawnScreenY - 8);
            ctx.restore();
        }

        // 6. Camera Reticle in exact center
        ctx.save();
        ctx.strokeStyle = 'rgba(71, 210, 201, 0.7)';
        ctx.lineWidth = 1.5;
        const cx = width / 2;
        const cy = height / 2;
        ctx.beginPath();
        ctx.moveTo(cx - 6, cy);
        ctx.lineTo(cx + 6, cy);
        ctx.moveTo(cx, cy - 6);
        ctx.lineTo(cx, cy + 6);
        ctx.stroke();
        ctx.restore();

        // 7. Paintbrush Chunks & Animated Shimmer Overlay
        // A. Loading Chunks (Sent to server): Darker cyan with animated loading gradient shimmer effect
        if (this.loadingChunks && this.loadingChunks.size > 0) {
            ctx.save();
            const time = performance.now();
            for (const [key, info] of this.loadingChunks.entries()) {
                const { cx, cz } = info;
                const chunkWorldX = cx * 16;
                const chunkWorldZ = cz * 16;
                const sX = width / 2 + (chunkWorldX - centerX) * zoom;
                const sY = height / 2 + (chunkWorldZ - centerZ) * zoom;
                const sSize = 16 * zoom;

                // Viewport cull check
                if (sX + sSize < -40 || sX > width + 40 || sY + sSize < -40 || sY > height + 40) continue;

                // Base darker cyan background
                ctx.fillStyle = 'rgba(6, 36, 46, 0.88)';
                ctx.fillRect(sX, sY, sSize, sSize);

                // Animated loading wave shimmer effect sweeping across connected chunks
                const wave = Math.sin((cx + cz) * 0.45 - time * 0.0055);
                const shimmerAlpha = 0.12 + 0.38 * Math.max(0, wave);
                ctx.fillStyle = `rgba(34, 211, 238, ${shimmerAlpha.toFixed(3)})`;
                ctx.fillRect(sX, sY, sSize, sSize);

                // Border in darker cyan
                ctx.strokeStyle = 'rgba(6, 182, 212, 0.75)';
                ctx.lineWidth = 1;
                ctx.strokeRect(sX, sY, sSize, sSize);

                // Center pulsating radar dot across all standard zoom levels
                if (sSize >= 4) {
                    ctx.fillStyle = '#38bdf8';
                    const baseRadius = Math.max(1.2, Math.min(sSize * 0.15, 3.5));
                    const dotPulse = baseRadius * (0.8 + 0.45 * Math.max(0, wave));
                    ctx.beginPath();
                    ctx.arc(sX + sSize / 2, sY + sSize / 2, dotPulse, 0, Math.PI * 2);
                    ctx.fill();
                }
            }
            ctx.restore();
        }

        // B. Active Painting Chunks: Lighter cyan highlight while brushing
        if (this.activePaintedChunks && this.activePaintedChunks.size > 0) {
            ctx.save();
            for (const key of this.activePaintedChunks) {
                const [cx, cz] = key.split(',').map(Number);
                const chunkWorldX = cx * 16;
                const chunkWorldZ = cz * 16;
                const sX = width / 2 + (chunkWorldX - centerX) * zoom;
                const sY = height / 2 + (chunkWorldZ - centerZ) * zoom;
                const sSize = 16 * zoom;

                if (sX + sSize < -40 || sX > width + 40 || sY + sSize < -40 || sY > height + 40) continue;

                // Lighter cyan surface
                ctx.fillStyle = 'rgba(34, 211, 238, 0.45)';
                ctx.fillRect(sX, sY, sSize, sSize);

                // Crisp lighter cyan border
                ctx.strokeStyle = 'rgba(103, 232, 249, 0.85)';
                ctx.lineWidth = 1;
                ctx.strokeRect(sX, sY, sSize, sSize);
            }
            ctx.restore();
        }

        // C. Hovering Brush Reticle under cursor
        if (this.mapTool === 'brush' && !this.mapIsDragging && this.previewHoverX !== undefined && this.previewHoverZ !== undefined) {
            // Hovering with Paintbrush: Show glowing chunk-snapped reticle
            ctx.save();
            const brushBounds = this.getBrushBounds(this.previewHoverX, this.previewHoverZ, this.brushSize || 1);
            const brushScreenX = width / 2 + (brushBounds.minX - centerX) * zoom;
            const brushScreenY = height / 2 + (brushBounds.minZ - centerZ) * zoom;
            const brushWidth = (brushBounds.maxX - brushBounds.minX + 1) * zoom;
            const brushHeight = (brushBounds.maxZ - brushBounds.minZ + 1) * zoom;

            // Brush highlight
            ctx.fillStyle = 'rgba(34, 211, 238, 0.16)';
            ctx.fillRect(brushScreenX, brushScreenY, brushWidth, brushHeight);

            ctx.strokeStyle = '#38bdf8';
            ctx.lineWidth = 1.5;
            ctx.setLineDash([4, 3]);
            ctx.strokeRect(brushScreenX, brushScreenY, brushWidth, brushHeight);

            // Crosshair at exact brush center
            const brushCenterX = brushScreenX + brushWidth / 2;
            const brushCenterY = brushScreenY + brushHeight / 2;
            ctx.setLineDash([]);
            ctx.strokeStyle = '#22d3ee';
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(brushCenterX - 5, brushCenterY);
            ctx.lineTo(brushCenterX + 5, brushCenterY);
            ctx.moveTo(brushCenterX, brushCenterY - 5);
            ctx.lineTo(brushCenterX, brushCenterY + 5);
            ctx.stroke();

            // Brush size & coordinates badge
            const chunkCount = Math.round((brushBounds.maxX - brushBounds.minX + 1) / 16);
            const brushLabel = this.brushSize === 32 
                ? `Region Stamp (r.${Math.floor(this.previewHoverX / 512)}.${Math.floor(this.previewHoverZ / 512)})`
                : `${chunkCount}x${chunkCount} Chunks (${chunkCount * 16}m)`;

            ctx.fillStyle = 'rgba(12, 18, 28, 0.88)';
            ctx.font = 'bold 9px monospace';
            const labelW = ctx.measureText(brushLabel).width + 14;
            ctx.fillRect(brushScreenX, brushScreenY - 20, labelW, 16);
            ctx.strokeStyle = 'rgba(56, 189, 248, 0.4)';
            ctx.lineWidth = 1;
            ctx.strokeRect(brushScreenX, brushScreenY - 20, labelW, 16);

            ctx.fillStyle = '#38bdf8';
            ctx.fillText(brushLabel, brushScreenX + 7, brushScreenY - 8);

            ctx.restore();
        } else {
            // Interactive Cursor Radar Exploration Ring (when in pan tool or Live Studio is active)
            const isRadarActive = ((this.selectedInstance && (this.selectedInstance.booted || this.selectedInstance.running)) || (this.mapStudioActive && this.warmWorkerReady));
            if (isRadarActive && !this.mapIsDragging && this.previewHoverX !== undefined && this.previewHoverZ !== undefined) {
                const cursorScreenX = (width / 2) + ((this.previewHoverX - centerX) * zoom);
                const cursorScreenY = (height / 2) + ((this.previewHoverZ - centerZ) * zoom);

                ctx.save();
                // Subtle dashed exploration radar boundary (144 blocks radius)
                ctx.strokeStyle = 'rgba(56, 189, 248, 0.45)';
                ctx.lineWidth = 1.25;
                ctx.setLineDash([4, 4]);
                ctx.beginPath();
                ctx.arc(cursorScreenX, cursorScreenY, Math.min(Math.max(144 * zoom, 28), 240), 0, Math.PI * 2);
                ctx.stroke();

                // Center cursor reticle
                ctx.setLineDash([]);
                ctx.strokeStyle = 'rgba(56, 189, 248, 0.75)';
                ctx.beginPath();
                ctx.moveTo(cursorScreenX - 5, cursorScreenY);
                ctx.lineTo(cursorScreenX + 5, cursorScreenY);
                ctx.moveTo(cursorScreenX, cursorScreenY - 5);
                ctx.lineTo(cursorScreenX + 5, cursorScreenY);
                ctx.stroke();
                ctx.restore();
            }
        }
    },

    async fetchRegionTile(rx, rz, force = false) {
        if (!this.selectedInstance) return;
        const instId = this.selectedInstance.id;
        const tileKey = `${instId}:${rx},${rz}`;
        this.mapPendingTiles.add(tileKey);

        try {
            const token = this.jwtToken || (function() {
                try { return localStorage.getItem('zircon.adminToken'); } catch (e) { return null; }
            })();
            const headers = {};
            if (token) headers['Authorization'] = `Bearer ${token}`;

            const shouldForce = force || this.mapForceTileRefresh;
            const seed = this.previewSeedInput || (this.serverProps && this.serverProps['level-seed']) || '';
            const seedParam = seed ? `&seed=${encodeURIComponent(seed)}` : '';
            const cacheBuster = shouldForce ? `&_t=${Date.now()}` : '';
            const url = `/api/instances/${instId}/world-map/tile?rx=${rx}&rz=${rz}${shouldForce ? '&force=true' : ''}${seedParam}${cacheBuster}`;
            
            console.log(`[Zircon WorldGen] Fetching tile for instance "${instId}" (r.${rx}.${rz}.mca) -> ${url}`);
            const res = await fetch(url, { headers });

            if (res.status === 200) {
                const blob = await res.blob();
                let img;
                if (typeof createImageBitmap !== 'undefined') {
                    img = await createImageBitmap(blob);
                } else {
                    img = new Image();
                    img.src = URL.createObjectURL(blob);
                    await img.decode();
                }
                this.mapTileCache.set(tileKey, img);
                if (this.mapGeneratingTiles) this.mapGeneratingTiles.delete(tileKey);

                // Release loading shimmer for chunks inside this newly updated region once worldgen generation window has elapsed
                if (this.loadingChunks && this.loadingChunks.size > 0) {
                    const now = Date.now();
                    for (const [coordKey, info] of this.loadingChunks.entries()) {
                        // Keep shimmer active during the initial 3.5s generation/flush window
                        if (info && info.startTime && (now - info.startTime < 3500)) {
                            continue;
                        }
                        const [cx, cz] = coordKey.split(',').map(Number);
                        const chRx = Math.floor((cx * 16) / 512);
                        const chRz = Math.floor((cz * 16) / 512);
                        if (chRx === rx && chRz === rz) {
                            this.loadingChunks.delete(coordKey);
                        }
                    }
                }

                console.log(`[Zircon WorldGen] Successfully loaded and rendered tile r.${rx}.${rz}.mca (${blob.size} bytes) for instance ${instId}`);
                this.addPreviewLog(`Loaded region tile r.${rx}.${rz}.mca`);
            } else {
                // 204 No Content or not found (unexplored void)
                console.log(`[Zircon WorldGen] Tile r.${rx}.${rz}.mca for instance ${instId} not yet generated (HTTP ${res.status})`);
                if (!this.mapGeneratingTiles || !this.mapGeneratingTiles.has(tileKey)) {
                    this.mapTileCache.set(tileKey, 'none');
                }
            }
        } catch (e) {
            console.warn(`[Zircon WorldGen] Failed to fetch tile r.${rx}.${rz}.mca for instance ${instId}:`, e);
            this.mapTileCache.set(tileKey, 'none');
        } finally {
            this.mapPendingTiles.delete(tileKey);
        }
    },

    async pregenerateSurrounding(radiusBlocks = 512) {
        if (!this.selectedInstance || this.pregenLoading) return;
        this.pregenLoading = true;
        this.previewStage = `Pre-generating terrain (+32 chunks / ${radiusBlocks}m)...`;
        console.log(`[Zircon WorldGen] Triggering pregeneration for instance "${this.selectedInstance.id}" at (${this.previewCenterX || 0}, ${this.previewCenterZ || 0}) radius ${radiusBlocks}m`);
        this.addPreviewLog(`Triggering pre-generation at (${this.previewCenterX || 0}, ${this.previewCenterZ || 0}) radius ${radiusBlocks}b...`);

        try {
            const seed = this.previewSeedInput || (this.serverProps && this.serverProps['level-seed']) || '';
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/pregenerate`, {
                method: 'POST',
                body: JSON.stringify({
                    radius_blocks: radiusBlocks,
                    center_x: this.previewCenterX || 0,
                    center_z: this.previewCenterZ || 0,
                    seed: seed || undefined
                })
            });

            console.log(`[Zircon WorldGen] Pregeneration worker response:`, res);
            this.addPreviewLog(`Pre-generation worker dispatched: ${res.message || 'Running in background'}`);
            if (this.showToast) {
                this.showToast(`Pre-generation dispatched (+32 chunks around ${this.previewCenterX || 0}, ${this.previewCenterZ || 0})`, 'info');
            }

            // Stream new tiles progressively: invalidate unexplored 'none' entries every 3s
            let attempts = 0;
            const pollInterval = setInterval(() => {
                attempts++;
                if (this.mapTileCache) {
                    for (let [key, val] of this.mapTileCache.entries()) {
                        if (val === 'none') {
                            this.mapTileCache.delete(key);
                        }
                    }
                }
                if (attempts >= 10) {
                    clearInterval(pollInterval);
                    this.pregenLoading = false;
                    this.previewStage = '';
                    this.addPreviewLog('Pre-generation background window complete.');
                }
            }, 3000);

        } catch (e) {
            console.error(`[Zircon WorldGen] Pre-generation request error:`, e);
            this.addPreviewLog(`Pre-generation request failed: ${e.message || e}`);
            this.pregenLoading = false;
            this.previewStage = '';
        }
    },

    // -------------------------------------------------------------------------
    // HUD & Navigation Controls
    // -------------------------------------------------------------------------
    zoomIn() {
        this.previewZoom = Math.min(parseFloat(((this.previewZoom || 1.0) * 1.3).toFixed(2)), 6.0);
    },

    zoomOut() {
        this.previewZoom = Math.max(parseFloat(((this.previewZoom || 1.0) / 1.3).toFixed(2)), 0.15);
    },

    recenterMap() {
        this.previewCenterX = 0;
        this.previewCenterZ = 0;
        this.previewZoom = 1.0;
        this.addPreviewLog("Camera recentered to World Origin (0, 0).");
    },

    toggleGrid() {
        this.previewShowGrid = !this.previewShowGrid;
    },

    setMapTool(tool) {
        this.mapTool = tool;
        if (window.Zircon && window.Zircon.preview) {
            window.Zircon.preview.mapTool = tool;
        }
        const canvas = document.getElementById('worldMapCanvas');
        if (canvas) {
            canvas.style.cursor = (tool === 'brush') ? 'crosshair' : 'grab';
        }
        this.addPreviewLog(`Tool switched to: ${tool === 'brush' ? 'Chunk Paintbrush' : 'Pan Camera'}`);
    },

    setBrushSize(sz) {
        this.brushSize = sz;
        if (window.Zircon && window.Zircon.preview) {
            window.Zircon.preview.brushSize = sz;
        }
        const labels = { 1: '1x1 (16m)', 3: '3x3 (48m)', 5: '5x5 (80m)', 32: 'Region (512m)' };
        this.addPreviewLog(`Brush size set to: ${labels[sz] || (sz + ' chunks')}`);
    },

    getBrushBounds(worldX, worldZ, size = 1) {
        if (size === 32) {
            // Region stamp (512x512 blocks)
            const rx = Math.floor(worldX / 512);
            const rz = Math.floor(worldZ / 512);
            return {
                minX: rx * 512,
                minZ: rz * 512,
                maxX: (rx + 1) * 512 - 1,
                maxZ: (rz + 1) * 512 - 1
            };
        }

        // Chunk grid (16x16 blocks)
        const cx = Math.floor(worldX / 16);
        const cz = Math.floor(worldZ / 16);
        const radius = Math.floor(size / 2); // 1 -> 0, 3 -> 1, 5 -> 2
        const minCx = cx - radius;
        const maxCx = cx + radius;
        const minCz = cz - radius;
        const maxCz = cz + radius;

        return {
            minX: minCx * 16,
            minZ: minCz * 16,
            maxX: (maxCx + 1) * 16 - 1,
            maxZ: (maxCz + 1) * 16 - 1
        };
    },

    addChunksAt(worldX, worldZ, size = 1) {
        if (!this.activePaintedChunks) this.activePaintedChunks = new Set();

        if (size === 32) {
            const rx = Math.floor(worldX / 512);
            const rz = Math.floor(worldZ / 512);
            const startCx = rx * 32;
            const startCz = rz * 32;
            for (let dx = 0; dx < 32; dx++) {
                for (let dz = 0; dz < 32; dz++) {
                    this.activePaintedChunks.add(`${startCx + dx},${startCz + dz}`);
                }
            }
            return;
        }

        const cx = Math.floor(worldX / 16);
        const cz = Math.floor(worldZ / 16);
        const radius = Math.floor(size / 2); // 1 -> 0, 3 -> 1, 5 -> 2
        for (let dx = -radius; dx <= radius; dx++) {
            for (let dz = -radius; dz <= radius; dz++) {
                this.activePaintedChunks.add(`${cx + dx},${cz + dz}`);
            }
        }
    },

    copyActiveSeed() {
        const seed = this.previewSeedInput || (this.serverProps && this.serverProps['level-seed']) || '';
        if (!seed) return;
        navigator.clipboard.writeText(seed).then(() => {
            this.previewCopiedSeed = true;
            setTimeout(() => { this.previewCopiedSeed = false; }, 2000);
            this.addPreviewLog(`Copied seed '${seed}' to clipboard.`);
        });
    },

    async renderMapFromCenter() {
        if (!this.selectedInstance || this.previewLoading || this.previewBooting || this.warmWorkerBooting) return;
        if (this.isServerOnline()) {
            await this.generatePreview();
        } else {
            await this.startMapStudio();
        }
    },

    async generatePreview() {
        if (!this.selectedInstance || this.previewLoading) return;
        this.previewLoading = true;
        this.previewStage = 'Scanning world region chunks...';
        console.log(`[Zircon WorldGen] generatePreview clicked for instance "${this.selectedInstance.id}" (${this.selectedInstance.name}). Requesting /api/instances/${this.selectedInstance.id}/preview-seed...`);
        this.addPreviewLog(`Initiating map render at (${this.previewCenterX || 0}, ${this.previewCenterZ || 0})...`);

        try {
            // Clear unexplored 'none' markers so new tiles stream fresh without flashing black
            if (this.mapTileCache) {
                for (const [key, val] of this.mapTileCache.entries()) {
                    if (val === 'none') {
                        this.mapTileCache.delete(key);
                    }
                }
            }
            this.mapForceTileRefresh = true;

            const postBody = {
                seed: this.previewSeedInput || '',
                center_x: this.previewCenterX || 0,
                center_z: this.previewCenterZ || 0,
                radius_chunks: this.previewRadiusChunks || 24,
                selected_mods: this.selectedTerrainMods || []
            };
            console.log(`[Zircon WorldGen] POST /api/instances/${this.selectedInstance.id}/preview-seed payload:`, postBody);

            const data = await this.api(`/api/instances/${this.selectedInstance.id}/preview-seed`, {
                method: 'POST',
                body: JSON.stringify(postBody)
            });

            console.log(`[Zircon WorldGen] Preview render result:`, data);
            this.previewResult = data;
            this.addPreviewLog(`Map rendered successfully in ${data.render_time_ms}ms (${data.biomes_found.length} biomes).`);

            // Immediately load center spawn tile
            const centerRx = Math.floor((this.previewCenterX || 0) / 512);
            const centerRz = Math.floor((this.previewCenterZ || 0) / 512);
            await this.fetchRegionTile(centerRx, centerRz, true);
        } catch (e) {
            console.error(`[Zircon WorldGen] Map render failed:`, e);
            this.addPreviewLog(`Map render failed: ${e.message || e}`);
        } finally {
            this.previewLoading = false;
            this.previewStage = '';
            setTimeout(() => { this.mapForceTileRefresh = false; }, 3000);
        }
    },

    randomizePreviewSeed() {
        const rand = Math.floor(Math.random() * 9007199254740991);
        this.previewSeedInput = rand.toString();
        this.addPreviewLog(`Generated new random seed: '${this.previewSeedInput}'`);
    },

    // -------------------------------------------------------------------------
    // Mod Management (Left Column)
    // -------------------------------------------------------------------------
    async fetchTerrainMods() {
        if (!this.selectedInstance) return;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/terrain-mods`);
            this.previewTerrainMods = data.candidates || [];

            // If not initialized, select likely terrain mods by default
            if (!this.selectedTerrainMods || this.selectedTerrainMods.length === 0) {
                this.selectedTerrainMods = this.previewTerrainMods
                    .filter(m => m.is_likely_terrain)
                    .map(m => m.file_name);
            }
            this.addPreviewLog(`Loaded ${this.previewTerrainMods.length} mods (${this.selectedTerrainMods.length} terrain mods enabled).`);
        } catch (e) {
            this.previewTerrainMods = [];
            this.addPreviewLog(`Failed to scan terrain mods: ${e.message || e}`);
        }
    },

    toggleTerrainMod(fileName) {
        if (!this.selectedTerrainMods) this.selectedTerrainMods = [];
        const idx = this.selectedTerrainMods.indexOf(fileName);
        if (idx > -1) {
            this.selectedTerrainMods.splice(idx, 1);
            this.addPreviewLog(`Excluded mod: ${fileName}`);
        } else {
            this.selectedTerrainMods.push(fileName);
            this.addPreviewLog(`Included mod: ${fileName}`);
        }
    },

    selectAllTerrainMods() {
        const allList = (this.previewTerrainMods || []).map(m => m.file_name);
        this.selectedTerrainMods = allList;
        this.addPreviewLog(`Selected all ${allList.length} mods.`);
    },

    selectTerrainOnly() {
        const terrainOnly = (this.previewTerrainMods || [])
            .filter(m => m.is_likely_terrain)
            .map(m => m.file_name);
        this.selectedTerrainMods = terrainOnly;
        this.addPreviewLog(`Filtered to ${terrainOnly.length} terrain generation mods.`);
    },

    deselectAllTerrainMods() {
        this.selectedTerrainMods = [];
        this.addPreviewLog("Deselected all mods (pure vanilla generation).");
    },

    isModSelected(fileName) {
        return (this.selectedTerrainMods || []).includes(fileName);
    },

    hasExistingPlayers() {
        return (this.players && this.players.length > 0) ||
               (this.playerHistory && this.playerHistory.length > 0);
    },

    // -------------------------------------------------------------------------
    // Seed Application & World Safety
    // -------------------------------------------------------------------------
    applyPreviewSeed() {
        if (!this.previewSeedInput) return;
        const newSeed = this.previewSeedInput.trim();
        const currentSeed = (this.serverProps && this.serverProps['level-seed']) || '';
        
        // Show red danger modal if world already has player history, rendered tiles, or existing level-seed
        const hasExistingWorld = this.hasExistingPlayers() || this.hasRenderedWorldMap() || (currentSeed && currentSeed !== newSeed);
        if (hasExistingWorld) {
            this.showSeedWarningModal = true;
            return;
        }
        this.confirmApplySeed();
    },

    cancelApplySeed() {
        this.showSeedWarningModal = false;
        this.addPreviewLog("Seed reset cancelled by user.");
    },

    promptWipeAndResetWorld() {
        this.randomizePreviewSeed();
        this.showSeedWarningModal = true;
    },

    async confirmApplySeed() {
        this.showSeedWarningModal = false;
        if (!this.selectedInstance || !this.previewSeedInput) return;
        const newSeed = this.previewSeedInput.trim();
        const instId = this.selectedInstance.id;

        // Clear client map caches immediately
        if (this.mapTileCache) this.mapTileCache.clear();
        if (this.mapPendingTiles) this.mapPendingTiles.clear();
        if (this.mapGeneratingTiles) this.mapGeneratingTiles.clear();
        if (this.activePaintedChunks) this.activePaintedChunks.clear();
        if (this.loadingChunks) this.loadingChunks.clear();
        this.previewCenterX = 0;
        this.previewCenterZ = 0;

        try {
            this.addPreviewLog(`Resetting world and applying new seed '${newSeed}'...`);
            if (this.showToast) {
                this.showToast(`Clearing world and applying seed '${newSeed}'...`, 'info');
            }

            // Call backend reset-world-seed endpoint to stop server, delete old world and caches, and set seed
            await this.api(`/api/instances/${instId}/world-map/reset-world-seed`, {
                method: 'POST',
                body: JSON.stringify({
                    seed: newSeed,
                    reboot: true
                })
            });

            if (this.serverProps) {
                this.serverProps['level-seed'] = newSeed;
            }
            this.addPreviewLog(`World reset complete! Booting engine with seed '${newSeed}'...`);
            if (this.showToast) {
                this.showToast(`World reset! Booting seed ${newSeed}...`, 'success');
            }

            // Await engine boot and stream new world spawn
            setTimeout(() => {
                this.bootServerForExploration();
            }, 1000);
        } catch (e) {
            this.addPreviewLog(`Failed to reset world: ${e.message || e}`);
            if (this.showToast) {
                this.showToast(`Failed to reset world: ${e.message || e}`, 'error');
            }
        }
    },

    resetWorldMapForInstance(newInstance) {
        const inst = newInstance || this.selectedInstance;
        if (!inst) return;
        if (this._lastResetInstanceId === inst.id) {
            return;
        }
        const oldInstanceId = this._lastResetInstanceId;
        this._lastResetInstanceId = inst.id;
        console.log(`[Zircon WorldGen] Switching/resetting map engine for instance:`, `${inst.name} (${inst.id})`);
        
        if (this.mapTileCache) {
            this.mapTileCache.clear();
        }
        if (this.mapPendingTiles) {
            this.mapPendingTiles.clear();
        }
        if (this.mapGeneratingTiles) {
            this.mapGeneratingTiles.clear();
        }

        // 2. Reset view position and zoom
        this.previewCenterX = 0;
        this.previewCenterZ = 0;
        this.previewZoom = 1.0;
        this.mapVelocityX = 0;
        this.mapVelocityZ = 0;

        // 3. Clear preview results and state
        this.previewResult = null;
        this.previewError = null;
        this.previewLoading = false;
        this.pregenLoading = false;
        this.previewStage = '';
        this.previewLogs = [];
        this.previewBooting = false;
        this.warmWorkerBooting = false;

        const isRunning = !!(inst && (inst.booted || inst.running));
        this.mapStudioActive = isRunning;
        this.warmWorkerReady = isRunning;

        // 4. Update seed input to match the instance
        if (this.serverProps && this.serverProps['level-seed']) {
            this.previewSeedInput = this.serverProps['level-seed'];
        } else {
            this.previewSeedInput = '';
        }

        this.addPreviewLog(`Switched map to instance: ${inst ? inst.name : 'Unknown'}`);

        // Stop any old warm session if switching away from previous instance
        if (oldInstanceId && oldInstanceId !== inst.id) {
            this.stopWarmMapSession(oldInstanceId);
        }

        // 5. If currently viewing the world tab, reload terrain mods and map engine
        if (this.activeTab === 'world') {
            this.fetchTerrainMods();
            this.$nextTick(() => {
                this.initWorldMapEngine();
            });
        }
    },

    // -------------------------------------------------------------------------
    // Unified Live Server Exploration & Viewport / Cursor Streaming
    // -------------------------------------------------------------------------
    async bootServerForExploration() {
        if (!this.selectedInstance || this.previewBooting) return;
        this.previewBooting = true;
        this.warmWorkerBooting = true;
        this.warmWorkerReady = false;

        const isSleeping = this.isServerSleeping();
        const actionLabel = isSleeping ? 'Waking server from sleep...' : 'Booting server for live exploration...';
        this.previewStage = actionLabel;
        this.addPreviewLog(isSleeping ? "Waking sleeping server for exploration..." : "Starting server engine for live exploration...");
        if (this.showToast) {
            this.showToast(isSleeping ? "Waking server from sleep..." : "Starting Minecraft server...", "info");
        }

        try {
            const seed = this.previewSeedInput || (this.serverProps && this.serverProps['level-seed']) || '';
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/world-map/boot-and-explore`, {
                method: 'POST',
                body: JSON.stringify({
                    seed: seed || undefined,
                    selected_mods: this.selectedTerrainMods || []
                })
            });

            this.addPreviewLog(res.message || "Server boot initiated.");

            // Poll instance state until running
            let attempts = 0;
            if (this._bootPollInterval) clearInterval(this._bootPollInterval);
            this._bootPollInterval = setInterval(async () => {
                attempts++;
                if (this.activeTab !== 'world' || !this.selectedInstance) {
                    clearInterval(this._bootPollInterval);
                    this._bootPollInterval = null;
                    this.previewBooting = false;
                    this.warmWorkerBooting = false;
                    return;
                }

                try {
                    if (typeof this.loadInstances === 'function') {
                        await this.loadInstances();
                    }
                    if (typeof this.refreshSelectedInstance === 'function') {
                        await this.refreshSelectedInstance();
                    }
                    if (this.isServerOnline()) {
                        clearInterval(this._bootPollInterval);
                        this._bootPollInterval = null;
                        this.previewBooting = false;
                        this.warmWorkerBooting = false;
                        this.warmWorkerReady = true;
                        this.mapStudioActive = true;
                        this.addPreviewLog("Server is ONLINE! Live world exploration active.");
                        if (this.showToast) {
                            this.showToast("Server is online! World exploration active.", "success");
                        }
                        if (this.pendingChunksToGenerate) {
                            const chunks = this.pendingChunksToGenerate;
                            this.pendingChunksToGenerate = null;
                            this.paintBatchChunks(chunks);
                        } else if (this.pendingAreaToGenerate) {
                            const { minX, minZ, maxX, maxZ } = this.pendingAreaToGenerate;
                            this.pendingAreaToGenerate = null;
                            this.paintChunksArea(minX, minZ, maxX, maxZ);
                        } else if (this.pendingRegionToGenerate) {
                            const { rx, rz } = this.pendingRegionToGenerate;
                            this.pendingRegionToGenerate = null;
                            this.generateSingleRegion(rx, rz);
                        } else {
                            // Initial generation around center/spawn (radius 384 blocks)
                            this.streamViewportArea(this.previewCenterX || 0, this.previewCenterZ || 0, 384);
                        }
                    }
                } catch (e) {
                    // keep polling
                }

                if (attempts >= 60) {
                    clearInterval(this._bootPollInterval);
                    this._bootPollInterval = null;
                    this.previewBooting = false;
                    this.warmWorkerBooting = false;
                    this.addPreviewLog("Server boot wait completed.");
                }
            }, 2000);

        } catch (e) {
            this.previewBooting = false;
            this.warmWorkerBooting = false;
            this.addPreviewLog(`Failed to start server: ${e.message || e}`);
            if (this.showToast) {
                this.showToast(`Failed to boot server: ${e.message || e}`, "error");
            }
        }
    },

    async startMapStudio() {
        if (!this.selectedInstance) return;
        if (this.isServerOnline()) {
            this.mapStudioActive = true;
            this.warmWorkerReady = true;
            this.warmWorkerBooting = false;
            this.previewBooting = false;
            this.addPreviewLog("Connected to running server for live exploration.");
            this.streamViewportArea(this.previewCenterX || 0, this.previewCenterZ || 0, 384);
            return;
        }

        // Server is offline or sleeping: boot the real server directly
        await this.bootServerForExploration();
    },

    async ensureWarmMapSession() {
        if (!this.selectedInstance) return;
        if (this.selectedInstance.booted || this.selectedInstance.running) {
            this.mapStudioActive = true;
            this.warmWorkerReady = true;
            this.warmWorkerBooting = false;
            this.previewBooting = false;
            return;
        }
        await this.bootServerForExploration();
    },

    async stopWarmMapSession(instanceId) {
        if (this._bootPollInterval) {
            clearInterval(this._bootPollInterval);
            this._bootPollInterval = null;
        }
        if (this._warmPollInterval) {
            clearInterval(this._warmPollInterval);
            this._warmPollInterval = null;
        }
        this.previewBooting = false;
        this.warmWorkerBooting = false;
        const id = instanceId || (this.selectedInstance && this.selectedInstance.id);
        if (!id) return;
        try {
            await this.api(`/api/instances/${id}/world-map/session/stop`, {
                method: 'POST'
            });
        } catch (e) {
            // best-effort cleanup
        }
    },

    isServerOnline() {
        const inst = this.selectedInstance;
        if (!inst) return false;
        // Java process MUST be currently running AND finished booting
        if (!inst.running || !inst.booted) return false;
        // Must not be in transitional or stopping or sleeping state
        if (inst.stopping || inst.booting || inst.wakeable) return false;
        if (typeof this.isFallingAsleep === 'function' && this.isFallingAsleep(inst)) return false;
        if (typeof this.isStopping === 'function' && this.isStopping(inst)) return false;
        return true;
    },

    isServerSleeping() {
        const inst = this.selectedInstance;
        if (!inst) return false;
        if (inst.wakeable) return true;
        if (typeof this.isFallingAsleep === 'function' && this.isFallingAsleep(inst)) return true;
        return false;
    },

    hasRenderedWorldMap() {
        if (!this.mapTileCache) return false;
        for (const [_, val] of this.mapTileCache.entries()) {
            if (val && val !== 'none') return true;
        }
        return false;
    },

    async generateSingleRegion(rx, rz) {
        const minX = rx * 512;
        const minZ = rz * 512;
        const maxX = (rx + 1) * 512 - 1;
        const maxZ = (rz + 1) * 512 - 1;
        return this.paintChunksArea(minX, minZ, maxX, maxZ);
    },

    async paintBatchChunks(chunks) {
        if (!this.selectedInstance || this.activeTab !== 'world' || !chunks || chunks.length === 0) return;

        const instId = this.selectedInstance.id;

        // Auto-wake or auto-boot if sleeping/offline
        if (!this.isServerOnline()) {
            const isSleeping = this.isServerSleeping();
            const actionText = isSleeping ? "Waking server from sleep" : "Starting server engine";
            this.addPreviewLog(`${actionText} to generate ${chunks.length} painted chunks...`);
            if (this.showToast) {
                this.showToast(`${actionText} to generate chunks...`, "info");
            }
            this.pendingChunksToGenerate = chunks;
            await this.bootServerForExploration();
            return;
        }

        // Determine all affected 512x512 regions
        const affectedRegionsMap = new Map();
        for (const ch of chunks) {
            const rx = Math.floor((ch.cx * 16) / 512);
            const rz = Math.floor((ch.cz * 16) / 512);
            const key = `${instId}:${rx},${rz}`;
            if (!affectedRegionsMap.has(key)) {
                affectedRegionsMap.set(key, { rx, rz, key });
            }
        }
        const affectedRegions = Array.from(affectedRegionsMap.values());

        console.log(`[Zircon WorldGen] Generating ${chunks.length} painted chunks across ${affectedRegions.length} regions for "${instId}"...`);
        this.addPreviewLog(`Generating ${chunks.length} painted chunks across ${affectedRegions.length} regions...`);

        try {
            const res = await this.api(`/api/instances/${instId}/world-map/generate-area`, {
                method: 'POST',
                body: JSON.stringify({ chunks })
            });

            console.log(`[Zircon WorldGen] Chunk batch dispatched:`, res);

            const regionsToPoll = (res && Array.isArray(res.regions) && res.regions.length > 0)
                ? res.regions
                : affectedRegions;

            const pollAffected = async (attempt = 1) => {
                if (!this.selectedInstance || this.selectedInstance.id !== instId) return;
                for (const r of regionsToPoll) {
                    await this.fetchRegionTile(r.rx, r.rz, true);
                }
                if (attempt < 10) {
                    setTimeout(() => pollAffected(attempt + 1), 1500);
                } else {
                    // Safety fallback: clear loading state for these chunks after all retries
                    if (this.loadingChunks) {
                        for (const ch of chunks) {
                            this.loadingChunks.delete(`${ch.cx},${ch.cz}`);
                        }
                    }
                }
            };

            setTimeout(() => pollAffected(1), 2500);

            // Safety timeout: clear loading markers after 20s if not already cleared
            setTimeout(() => {
                if (this.loadingChunks) {
                    for (const ch of chunks) {
                        this.loadingChunks.delete(`${ch.cx},${ch.cz}`);
                    }
                }
            }, 20000);

            this.addPreviewLog(`Painted batch of ${chunks.length} chunks scheduled.`);
        } catch (e) {
            console.warn(`[Zircon WorldGen] Failed to generate chunk batch:`, e);
            this.addPreviewLog(`Failed to generate chunks: ${e.message || e}`);
            if (this.loadingChunks) {
                for (const ch of chunks) {
                    this.loadingChunks.delete(`${ch.cx},${ch.cz}`);
                }
            }
        }
    },

    async paintChunksArea(minX, minZ, maxX, maxZ) {
        if (!this.selectedInstance || this.activeTab !== 'world') return;

        const instId = this.selectedInstance.id;

        // Auto-wake or auto-boot if sleeping/offline
        if (!this.isServerOnline()) {
            const isSleeping = this.isServerSleeping();
            const actionText = isSleeping ? "Waking server from sleep" : "Starting server engine";
            this.addPreviewLog(`${actionText} to generate chunks...`);
            if (this.showToast) {
                this.showToast(`${actionText} to generate chunks...`, "info");
            }
            this.pendingAreaToGenerate = { minX, minZ, maxX, maxZ };
            await this.bootServerForExploration();
            return;
        }

        // Determine all affected 512x512 regions
        const minRx = Math.floor(minX / 512);
        const maxRx = Math.floor(maxX / 512);
        const minRz = Math.floor(minZ / 512);
        const maxRz = Math.floor(maxZ / 512);

        if (!this.mapGeneratingTiles) this.mapGeneratingTiles = new Set();
        const affectedRegions = [];
        for (let rx = minRx; rx <= maxRx; rx++) {
            for (let rz = minRz; rz <= maxRz; rz++) {
                const tileKey = `${instId}:${rx},${rz}`;
                this.mapGeneratingTiles.add(tileKey);
                affectedRegions.push({ rx, rz, tileKey });
            }
        }

        const chunkW = Math.round((maxX - minX + 1) / 16);
        const chunkH = Math.round((maxZ - minZ + 1) / 16);
        console.log(`[Zircon WorldGen] Generating chunks (${minX}, ${minZ}) to (${maxX}, ${maxZ}) [${chunkW}x${chunkH} chunks, ${affectedRegions.length} regions]`);
        this.addPreviewLog(`Generating ${chunkW}x${chunkH} chunks (${minX}, ${minZ} to ${maxX}, ${maxZ})...`);

        try {
            const res = await this.api(`/api/instances/${instId}/world-map/generate-area`, {
                method: 'POST',
                body: JSON.stringify({ minX, minZ, maxX, maxZ })
            });

            console.log(`[Zircon WorldGen] Chunk generation scheduled:`, res);

            const regionsToPoll = (res && Array.isArray(res.regions) && res.regions.length > 0)
                ? res.regions
                : affectedRegions;

            const pollAffected = async (attempt = 1) => {
                if (!this.selectedInstance || this.selectedInstance.id !== instId) return;
                let anyStillPending = false;
                for (const r of regionsToPoll) {
                    const key = `${instId}:${r.rx},${r.rz}`;
                    await this.fetchRegionTile(r.rx, r.rz, true);
                    const updated = this.mapTileCache ? this.mapTileCache.get(key) : null;
                    if (updated && updated !== 'none') {
                        if (this.mapGeneratingTiles) this.mapGeneratingTiles.delete(key);
                    } else {
                        anyStillPending = true;
                    }
                }
                if (anyStillPending && attempt < 5) {
                    setTimeout(() => pollAffected(attempt + 1), 1500);
                } else {
                    for (const r of regionsToPoll) {
                        const key = `${instId}:${r.rx},${r.rz}`;
                        if (this.mapGeneratingTiles) this.mapGeneratingTiles.delete(key);
                    }
                }
            };

            setTimeout(() => pollAffected(1), 1600);
        } catch (e) {
            console.warn(`[Zircon WorldGen] Failed to generate chunk area:`, e);
            this.addPreviewLog(`Failed to generate chunks: ${e.message || e}`);
            for (const r of affectedRegions) {
                if (this.mapGeneratingTiles) this.mapGeneratingTiles.delete(r.tileKey);
            }
        }
    },

    async streamCurrentViewport() {
        if (!this.selectedInstance || this.activeTab !== 'world') return;
        const isLive = (this.selectedInstance.booted || this.selectedInstance.running) || this.warmWorkerReady || this.mapStudioActive;
        if (!isLive) return;

        const canvas = document.getElementById('worldMapCanvas');
        if (!canvas) return;

        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return;

        const zoom = this.previewZoom || 1.0;
        const centerX = this.previewCenterX || 0;
        const centerZ = this.previewCenterZ || 0;

        const halfW = rect.width / (2 * zoom);
        const halfH = rect.height / (2 * zoom);

        // Compute viewport generation area based on visible canvas (up to 768 blocks radius)
        const radius = Math.min(Math.max(halfW, halfH, 256), 768);
        await this.streamViewportArea(centerX, centerZ, radius);
    },

    async streamViewportArea(centerX, centerZ, radius = 256) {
        if (!this.selectedInstance || this.activeTab !== 'world') return;
        const isLive = (this.selectedInstance.booted || this.selectedInstance.running) || this.warmWorkerReady || this.mapStudioActive;
        if (!isLive) return;

        const minX = Math.round(centerX - radius);
        const minZ = Math.round(centerZ - radius);
        const maxX = Math.round(centerX + radius);
        const maxZ = Math.round(centerZ + radius);

        const instId = this.selectedInstance.id;
        const minRx = Math.floor(minX / 512);
        const maxRx = Math.floor(maxX / 512);
        const minRz = Math.floor(minZ / 512);
        const maxRz = Math.floor(maxZ / 512);

        if (!this.mapGeneratingTiles) this.mapGeneratingTiles = new Set();

        // Check if all region tiles in this boundary are already cached and loaded
        const unrenderedRegions = [];
        for (let rx = minRx; rx <= maxRx; rx++) {
            for (let rz = minRz; rz <= maxRz; rz++) {
                const tileKey = `${instId}:${rx},${rz}`;
                const cached = this.mapTileCache ? this.mapTileCache.get(tileKey) : undefined;
                if (!cached || cached === 'none') {
                    if (!this.mapGeneratingTiles.has(tileKey)) {
                        unrenderedRegions.push({ rx, rz, tileKey });
                    }
                }
            }
        }

        if (unrenderedRegions.length === 0) {
            return;
        }

        // Mark regions as actively generating so they immediately render dark cyan with spinners
        for (const r of unrenderedRegions) {
            this.mapGeneratingTiles.add(r.tileKey);
        }

        console.log(`[Zircon WorldGen] Streaming bounds for "${instId}": (${minX}, ${minZ}) -> (${maxX}, ${maxZ}) [${unrenderedRegions.length} regions]`);

        try {
            const res = await this.api(`/api/instances/${instId}/world-map/generate-area`, {
                method: 'POST',
                body: JSON.stringify({
                    minX,
                    minZ,
                    maxX,
                    maxZ
                })
            });

            console.log(`[Zircon WorldGen] Chunk generation response:`, res.message);

            // Poll for the freshly generated & flushed region files without deleting old cache
            const pollRegions = async (attempt = 1) => {
                if (!this.selectedInstance || this.selectedInstance.id !== instId) return;
                let anyStillPending = false;
                for (const r of unrenderedRegions) {
                    const tileKey = r.tileKey;
                    const cached = this.mapTileCache.get(tileKey);
                    if (cached && cached !== 'none') {
                        this.mapGeneratingTiles.delete(tileKey);
                        continue;
                    }
                    await this.fetchRegionTile(r.rx, r.rz, true);
                    const updated = this.mapTileCache.get(tileKey);
                    if (updated && updated !== 'none') {
                        this.mapGeneratingTiles.delete(tileKey);
                    } else {
                        anyStillPending = true;
                    }
                }
                if (anyStillPending && attempt < 4) {
                    setTimeout(() => pollRegions(attempt + 1), 1500);
                } else {
                    for (const r of unrenderedRegions) {
                        this.mapGeneratingTiles.delete(r.tileKey);
                    }
                }
            };

            setTimeout(() => pollRegions(1), 1600);
        } catch (e) {
            console.warn(`[Zircon WorldGen] Failed to generate area:`, e);
            for (const r of unrenderedRegions) {
                this.mapGeneratingTiles.delete(r.tileKey);
            }
        }
    },

    addPreviewLog(msg) {
        if (!this.previewLogs) this.previewLogs = [];
        const time = new Date().toLocaleTimeString();
        this.previewLogs.push(`[${time}] ${msg}`);
        if (this.previewLogs.length > 80) this.previewLogs.shift();
    }
};
