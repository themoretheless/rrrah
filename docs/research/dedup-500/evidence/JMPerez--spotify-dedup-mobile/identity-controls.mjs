import assert from "node:assert/strict";
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


let token;
let api;

class Deduplicator {
  constructor(api, promiseThrottle) {
    this.api = api;
    this.promiseThrottle = promiseThrottle;
  }

  async findDuplicates(playlist) {
    const seenIds = {};
    const seenNameAndArtist = {};
    const duplicates = [];
    return new Promise((resolve, reject) =>
      promisesForPages(
        this.promiseThrottle.add(() =>
          this.api.getGeneric(playlist.tracks.href)
        ),
        this.promiseThrottle,
        this.api
      )
        .then((
          pagePromises // todo: I'd love to replace this with
        ) =>
          // .then(Promise.all)
          // à la http://www.html5rocks.com/en/tutorials/es6/promises/#toc-transforming-values
          Promise.all(pagePromises)
        )
        .then(pages => {
          pages.forEach(page => {
            const pageOffset = page.offset;
            page.items.forEach((item, index) => {
              if (item.track.id !== null) {
                let isDuplicate = false;
                const seenNameAndArtistKey = `${item.track.name}:${
                  item.track.artists[0].name
                }`;
                if (item.track.id in seenIds) {
                  // if the two items have the same Spotify ID, they are duplicates
                  isDuplicate = true;
                } else {
                  // if they have the same name, main artist, and roughly same duration
                  // we consider tem duplicates too
                  if (
                    seenNameAndArtistKey in seenNameAndArtist &&
                    Math.abs(
                      seenNameAndArtist[seenNameAndArtistKey] -
                        item.track.duration_ms
                    ) < 2000
                  ) {
                    isDuplicate = true;
                  }
                }
                if (isDuplicate) {
                  duplicates.push({
                    index: pageOffset + index,
                    track: item.track,
                    reason:
                      item.track.id in seenIds ? 'same-id' : 'same-name-artist'
                  });
                } else {
                  seenIds[item.track.id] = true;
                  seenNameAndArtist[seenNameAndArtistKey] =
                    item.track.duration_ms;
                }
              }
            });
          });
          resolve(duplicates);
        })
        .catch(reject)
    );
  }

  async removeDuplicates(playlist) {
    return new Promise((resolve, reject) => {
      if (playlist.id === 'starred') {
        reject(
          'It is not possible to delete duplicates from your Starred playlist using this tool since this is not supported in the Spotify Web API. You will need to remove these manually.'
        );
      }
      if (playlist.collaborative) {
        reject(
          'It is not possible to delete duplicates from a collaborative playlist using this tool since this is not supported in the Spotify Web API. You will need to remove these manually.'
        );
      } else {
        this.promiseThrottle.add(async () => {
          const tracksToRemove = playlist.duplicates.map(d => ({
            uri: d.track.linked_from ? d.track.linked_from.uri : d.track.uri,
            positions: [d.index]
          }));

          // remove chunks of max 100 tracks
          // find again duplicated tracks
          // delete another chunk

          const chunk = tracksToRemove.splice(0, 100);

          const result = await this.api.removeTracksFromPlaylist(
            playlist.owner.id,
            playlist.id,
            chunk
          );
          const duplicates = await this.findDuplicates(playlist);
          if (duplicates.length > 0) {
            this.removeDuplicates(playlist);
          } else {
            resolve();
          }
        });
      }
    });
  }
}
const throttle={add:fn=>Promise.resolve().then(fn)};
const tr=(id,name='same',ms=10000,artist='artist')=>({id,name,duration_ms:ms,artists:[{name:artist}],uri:'synthetic:'+id});
const scan=items=>new Deduplicator({async getGeneric(){return {items:items.map(track=>({track})),offset:0,limit:100,total:items.length,href:'synthetic'}}},throttle).findDuplicates({tracks:{href:'synthetic'}});
assert.equal((await scan([tr('toString')]))[0].reason,'same-id');
assert.equal((await scan([tr('a'),tr('b'),tr('b','other',20000)])).length,1);
await assert.rejects(scan([null]),TypeError);
assert.equal((await scan([tr('a'),tr('b','same',11999)])).length,1);assert.equal((await scan([tr('a'),tr('b','same',12000)])).length,0);
console.log(JSON.stringify({controls:4,passed:4,observations:['prototype key toString falsely marked duplicate in singleton','heuristic duplicate ID not recorded; later same ID with changed metadata escapes','null track rejects entire scan','duration tolerance strictly less than2000ms'],scope:'original Deduplicator and pagination bodies; import/export adapters, synthetic API and immediate throttle; no removal method invoked, Spotify/account/network untouched'}));
