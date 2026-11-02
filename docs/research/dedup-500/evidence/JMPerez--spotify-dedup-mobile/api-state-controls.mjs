import assert from "node:assert/strict";
const apiPrefix = 'https://api.spotify.com/v1';

class SpotifyWebApi {
  constructor() {
    this.token = null;
  }
  setAccessToken(token) {
    this.token = token;
  }
  async getMe() {
    return await this.getGeneric(`${apiPrefix}/me`);
  }

  async getGeneric(url, options) {
    const optionsString =
      options === undefined
        ? ''
        : `&${Object.keys(options)
            .map(k => `${k}=${options[k]}`)
            .join('=')}`;

    const res = await fetch(`${url}${optionsString}`, {
      method: 'GET',
      headers: {
        Authorization: `Bearer ${this.token}`
      }
    });
    const json = await res.json();
    if (res.ok) return json;
    return null;
  }

  async getUserPlaylists(userId, options) {
    const url =
      typeof userId === 'string'
        ? `${apiPrefix}/users/${encodeURIComponent(userId)}/playlists`
        : `${apiPrefix}/me/playlists`;
    return await this.getGeneric(url, options);
  }

  async removeTracksFromPlaylist(userId, playlistId, uris) {
    const dataToBeSent = {
      tracks: uris.map(uri => (typeof uri === 'string' ? { uri: uri } : uri))
    };

    const res = await fetch(
      `${apiPrefix}/users/${encodeURIComponent(
        userId
      )}/playlists/${playlistId}/tracks`,
      {
        method: 'DELETE',
        headers: {
          Authorization: `Bearer ${this.token}`
        },
        body: JSON.stringify(dataToBeSent)
      }
    );

    const json = await res.json();
    if (res.ok) return json;
    return null;
  }
}



async function promisesForPages(promise, promiseThrottle, api) {
  function stripParameters(href) {
    return href.indexOf('?') !== -1 ? href.substr(0, href.indexOf('?')) : href;
  }

  async function fetchGeneric(results, offset, limit) {
    return api.getGeneric(
      `${stripParameters(results.href)}?offset=${offset}&limit=${limit}`
    );
  }

  const results = await promise;
  if (results === null) {
    return [];
  }
  const promises = [promise];
  let offset = results.limit + results.offset;
  const limit = results.limit;
  while (results.total > offset) {
    const q = promiseThrottle.add(
      fetchGeneric.bind(this, results, offset, limit)
    );
    promises.push(q);
    offset += limit;
  }
  return promises;
}

const STATES = {
  LOADING: 0,
  FINDING_DUPLICATES: 1,
  DONE: 2
};

const initialState = { items: [], libraryState: STATES.LOADING };
function playlistsReducer(
  state = initialState,
  action
) {
  switch (action.type) {
    case FETCH_PLAYLISTS: {
      return initialState;
    }
    case FETCH_PLAYLISTS_SUCCESS: {
      return {
        ...state,
        items: action.playlists,
        libraryState: STATES.FINDING_DUPLICATES
      };
    }
    case FIND_DUPLICATES: {
      return {
        ...state,
        items: state.items.map(playlist => {
          return { ...playlist, duplicates: [], processing: true };
        }),
        libraryState: STATES.FINDING_DUPLICATES
      };
    }
    case FIND_DUPLICATES_IN_PLAYLIST: {
      return { ...state, processing: action.playlist };
    }
    case FIND_DUPLICATES_IN_PLAYLIST_SUCCESS: {
      return {
        ...state,
        processing: null,
        items: state.items.map(playlist => {
          if (playlist.uri === action.playlist.uri) {
            return {
              ...playlist,
              duplicates: action.duplicates,
              processing: false
            };
          }
          return playlist;
        })
      };
    }
    case FIND_DUPLICATES_SUCCESS: {
      return { ...state, libraryState: STATES.DONE };
    }
    case REMOVE_DUPLICATES_SUCCESS: {
      return {
        ...state,
        items: state.items.map(playlist => {
          if (playlist.uri === action.playlist.uri) {
            return { ...playlist, duplicates: [] };
          }
          return playlist;
        })
      };
    }
    default:
      return state;
  }
}
const FETCH_PLAYLISTS = 'FETCH_PLAYLISTS';
function fetchPlaylists() {
  return {
    type: FETCH_PLAYLISTS
  };
}

const FETCH_PLAYLISTS_SUCCESS = 'FETCH_PLAYLISTS_SUCCESS';
function fetchPlaylistsSuccess(playlists) {
  return {
    type: FETCH_PLAYLISTS_SUCCESS,
    playlists
  };
}

const FIND_DUPLICATES = 'FIND_DUPLICATES';
const FIND_DUPLICATES_SUCCESS = 'FIND_DUPLICATES_SUCCESS';
const FIND_DUPLICATES_IN_PLAYLIST = 'FIND_DUPLICATES_IN_PLAYLIST';
const FIND_DUPLICATES_IN_PLAYLIST_SUCCESS =
  'FIND_DUPLICATES_IN_PLAYLIST_SUCCESS';

function findDuplicates() {
  return {
    type: FIND_DUPLICATES
  };
}
function findDuplicatesSuccess() {
  return {
    type: FIND_DUPLICATES_SUCCESS
  };
}

function findDuplicatesInPlaylist(playlist) {
  return {
    type: FIND_DUPLICATES_IN_PLAYLIST,
    playlist
  };
}

function findDuplicatesInPlaylistSuccess(playlist, duplicates) {
  return {
    type: FIND_DUPLICATES_IN_PLAYLIST_SUCCESS,
    playlist,
    duplicates
  };
}

const REMOVE_DUPLICATES = 'REMOVE_DUPLICATES';
const REMOVE_DUPLICATES_SUCCESS = 'REMOVE_DUPLICATES_SUCCESS';
function removeDuplicates(playlist) {
  // should this be an async action returning a promise?
  return {
    type: REMOVE_DUPLICATES,
    playlist
  };
}
function removeDuplicatesSuccess(playlist) {
  return {
    type: REMOVE_DUPLICATES_SUCCESS,
    playlist
  };
}

// selectors
const getPlaylists = state =>
  (state.playlists && state.playlists.items) || [];
const getTotalDuplicates = state =>
  state.reduce((a, p) => a + p.duplicates.length, 0);
const getPlaylistsWithDuplicates = state =>
  state.playlists.items.filter(i => i.duplicates && i.duplicates.length > 0) ||
  [];
const getProgress = state => {
  if (state.playlists.items.length) {
    const processing = state.playlists.items.filter(i => i.processing).length;
    const total = state.playlists.items.length;
    return total === 0 ? 100 : 100 - processing * 100 / total;
  }
  return 0;
};

const urls=[];globalThis.fetch=async url=>{urls.push(url);return {ok:false,json:async()=>({error:'synthetic failure'})}};
const api=new SpotifyWebApi();const res=await api.getGeneric('synthetic');assert.equal(res,null);assert.deepEqual(await promisesForPages(Promise.resolve(res),{add:fn=>fn()},api),[]);
await api.getUserPlaylists({limit:50});assert.equal(urls.at(-1),'https://api.spotify.com/v1/me/playlists');
const state={items:[{id:'a',duplicates:[1]},{id:'b',duplicates:[2]}],libraryState:STATES.FINDING_DUPLICATES};const updated=playlistsReducer(state,findDuplicatesInPlaylistSuccess({id:'a'},[99]));assert.deepEqual(updated.items.map(p=>p.duplicates),[[99],[99]]);
console.log(JSON.stringify({controls:3,passed:3,observations:['non-ok API response null becomes successful zero pages','getUserPlaylists object argument ignores intended limit','missing URI makes reducer update multiple different playlist IDs'],scope:'original API/paginator/reducer bodies with export adapter; fetch fully stubbed; no real API/removal/account access'}));
