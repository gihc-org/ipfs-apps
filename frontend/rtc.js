// Loft mesh-WebRTC — ét RTCPeerConnection pr. anden deltager.
//
// Perfect negotiation: den ene side (selfId < peerId) er "polite" og
// håndterer offer-glare via rollback; den anden er impolite og dropper
// konkurrerende offers. Se ADR-draft 0028.

class LoftRTC {
  constructor(opts) {
    this.ws = opts.ws;
    this.selfId = opts.selfId;
    this.iceServers = opts.iceServers || [];
    this.onRemoteTrack = opts.onRemoteTrack || (() => {});
    this.onPeerState = opts.onPeerState || (() => {});
    this.log = opts.log || ((...args) => console.debug('[rtc]', ...args));
    this.peers = new Map();
    this.local = new Map(); // tag -> MediaStream
  }

  ensurePeer(participant) {
    const existing = this.peers.get(participant.id);
    if (existing) {
      existing.name = participant.name || existing.name;
      return existing;
    }

    const pc = new RTCPeerConnection({ iceServers: this.iceServers });
    const peer = {
      id: participant.id,
      name: participant.name || 'Deltager',
      pc,
      polite: String(this.selfId) < String(participant.id),
      makingOffer: false,
      ignoreOffer: false,
      remoteStream: new MediaStream(),
    };

    pc.onicecandidate = (e) => {
      if (e.candidate) {
        this._sendSignal(peer, { type: 'ice-candidate', candidate: e.candidate });
      }
    };

    pc.ontrack = (e) => {
      peer.remoteStream.addTrack(e.track);
      this.onRemoteTrack(peer, e.track);
    };

    pc.onconnectionstatechange = () => {
      this.onPeerState(peer, pc.connectionState);
    };

    pc.onnegotiationneeded = () => {
      this._negotiate(peer);
    };

    // En ny peer skal have de streams vi allerede deler.
    for (const stream of this.local.values()) {
      for (const track of stream.getTracks()) {
        pc.addTrack(track, stream);
      }
    }

    this.peers.set(peer.id, peer);
    this.onPeerState(peer, 'new');
    return peer;
  }

  handleSignal(fromId, signal) {
    const peer = this.peers.get(fromId);
    if (!peer) {
      this.log('signal fra ukendt deltager', fromId, signal.type);
      return;
    }

    const pc = peer.pc;
    if (signal.type === 'offer') {
      this._handleOffer(peer, signal.sdp);
    } else if (signal.type === 'answer') {
      if (pc.signalingState !== 'stable') {
        pc.setRemoteDescription(signal.sdp).catch((e) => {
          this.log('kunne ikke anvende answer', e);
        });
      }
    } else if (signal.type === 'ice-candidate') {
      if (signal.candidate) {
        pc.addIceCandidate(signal.candidate).catch((e) => {
          this.log('kunne ikke tilføje ICE-kandidat', e);
        });
      }
    } else if (signal.type === 'close') {
      this.removePeer(fromId);
    }
  }

  addMedia(tag, stream) {
    this.removeMedia(tag, false); // erstat evt. tidligere stream med samme tag
    this.local.set(tag, stream);
    for (const peer of this.peers.values()) {
      for (const track of stream.getTracks()) {
        const exists = peer.pc.getSenders().some((s) => s.track === track);
        if (!exists) peer.pc.addTrack(track, stream);
      }
    }
    // addTrack/removeTrack udløser onnegotiationneeded pr. peer.
  }

  removeMedia(tag, stopTracks = true) {
    const stream = this.local.get(tag);
    if (!stream) return;
    const tracks = stream.getTracks();
    for (const peer of this.peers.values()) {
      for (const sender of peer.pc.getSenders()) {
        if (tracks.includes(sender.track)) {
          peer.pc.removeTrack(sender);
        }
      }
    }
    if (stopTracks) tracks.forEach((t) => t.stop());
    this.local.delete(tag);
  }

  removePeer(peerId) {
    const peer = this.peers.get(peerId);
    if (!peer) return;
    peer.pc.close();
    this.peers.delete(peerId);
    this.onPeerState(peer, 'closed');
  }

  closeAll() {
    for (const id of [...this.peers.keys()]) {
      this.removePeer(id);
    }
  }

  _sendSignal(peer, signal) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: 'signal', to: peer.id, signal }));
    }
  }

  async _negotiate(peer) {
    const pc = peer.pc;
    try {
      peer.makingOffer = true;
      await pc.setLocalDescription();
      this._sendSignal(peer, { type: 'offer', sdp: pc.localDescription });
    } catch (e) {
      this.log('negotiation fejlede', e);
      try {
        if (pc.signalingState !== 'stable') {
          await pc.setLocalDescription({ type: 'rollback' });
        }
      } catch (_) {
        /* rollback er best effort */
      }
    } finally {
      peer.makingOffer = false;
    }
  }

  async _handleOffer(peer, description) {
    const pc = peer.pc;
    const collision = peer.makingOffer || pc.signalingState !== 'stable';
    if (collision && !peer.polite) {
      peer.ignoreOffer = true;
      return;
    }
    try {
      if (collision) {
        await Promise.all([
          pc.setLocalDescription({ type: 'rollback' }),
          pc.setRemoteDescription(description),
        ]);
      } else {
        await pc.setRemoteDescription(description);
      }
      await pc.setLocalDescription();
      this._sendSignal(peer, { type: 'answer', sdp: pc.localDescription });
    } catch (e) {
      this.log('offer-håndtering fejlede', e);
    }
  }
}

// Plain script (ikke ES-modul) — tilgængelig fra loft.html.
window.LoftRTC = LoftRTC;
