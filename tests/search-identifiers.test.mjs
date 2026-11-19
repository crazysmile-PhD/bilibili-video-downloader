import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  extractAid,
  extractBvid,
  extractEpId,
  extractSeasonId,
  extractUid,
  extractMediaListId,
} from '../src/features/search/identifiers.ts'

test('extracts supported video, episode, season, user and favorite identifiers', () => {
  assert.equal(extractAid('https://www.bilibili.com/video/av12345?p=2'), 12345)
  assert.equal(extractBvid('https://www.bilibili.com/video/BV1xx411c7mD'), 'BV1xx411c7mD')
  assert.equal(extractEpId('https://www.bilibili.com/bangumi/play/ep123'), 123)
  assert.equal(extractSeasonId('https://www.bilibili.com/bangumi/play/ss456'), 456)
  assert.equal(extractUid('https://space.bilibili.com/789/video'), 789)
  assert.equal(extractMediaListId('https://space.bilibili.com/789/favlist?fid=321'), 321)
})

test('rejects malformed numeric identifiers instead of accepting a numeric prefix', () => {
  for (const value of ['123abc', '-1', '0', '9007199254740993', '']) {
    assert.equal(extractAid(`https://www.bilibili.com/video/av${value}`), undefined)
    assert.equal(extractMediaListId(`https://space.bilibili.com/1/favlist?fid=${value}`), undefined)
  }
})

test('invalid URLs and unrelated paths consistently return undefined', () => {
  for (const extract of [extractAid, extractBvid, extractEpId, extractSeasonId, extractUid, extractMediaListId]) {
    assert.equal(extract('not a URL'), undefined)
    assert.equal(extract('https://example.com/'), undefined)
  }
  assert.equal(extractUid('https://example.com/123'), undefined)
})
