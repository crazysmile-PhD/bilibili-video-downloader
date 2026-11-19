function parseUrl(value: string): URL | undefined {
  try {
    return new URL(value)
  } catch {
    return undefined
  }
}

function numericId(value: string | null | undefined): number | undefined {
  if (!value || !/^\d+$/.test(value)) return undefined
  const id = Number(value)
  return Number.isSafeInteger(id) && id > 0 ? id : undefined
}

function pathId(url: string, prefix: string): string | undefined {
  const segments = parseUrl(url)?.pathname.split('/') ?? []
  return segments.find((segment) => segment.toLowerCase().startsWith(prefix))?.slice(prefix.length)
}

export function extractAid(url: string): number | undefined {
  return numericId(pathId(url, 'av'))
}

export function extractBvid(url: string): string | undefined {
  return parseUrl(url)
    ?.pathname.split('/')
    .find((segment) => /^bv[0-9a-z]+$/i.test(segment))
}

export function extractEpId(url: string): number | undefined {
  return numericId(pathId(url, 'ep'))
}

export function extractSeasonId(url: string): number | undefined {
  return numericId(pathId(url, 'ss'))
}

export function extractUid(url: string): number | undefined {
  const parsed = parseUrl(url)
  return parsed?.hostname === 'space.bilibili.com' ? numericId(parsed.pathname.split('/')[1]) : undefined
}

export function extractMediaListId(url: string): number | undefined {
  return numericId(parseUrl(url)?.searchParams.get('fid'))
}
