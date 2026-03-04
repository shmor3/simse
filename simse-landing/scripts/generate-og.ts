import { writeFile } from 'node:fs/promises';
import { Resvg } from '@resvg/resvg-js';
import satori from 'satori';

// Fetch DM Sans Bold from Google Fonts (request TTF via user-agent)
const cssResponse = await fetch(
	'https://fonts.googleapis.com/css2?family=DM+Sans:wght@700',
	{ headers: { 'User-Agent': 'Mozilla/5.0 AppleWebKit/537.36' } },
);
const css = await cssResponse.text();
const fontUrl = css.match(/url\(([^)]+)\)/)?.[1];
if (!fontUrl) throw new Error('Could not find font URL in Google Fonts CSS');
const fontResponse = await fetch(fontUrl);
const fontData = await fontResponse.arrayBuffer();

const svg = await satori(
	{
		type: 'div',
		props: {
			style: {
				width: '100%',
				height: '100%',
				display: 'flex',
				flexDirection: 'column',
				justifyContent: 'center',
				alignItems: 'flex-start',
				backgroundColor: '#0a0a0b',
				padding: '80px 100px',
				fontFamily: 'DM Sans',
			},
			children: [
				{
					type: 'div',
					props: {
						style: {
							display: 'flex',
							alignItems: 'center',
							gap: '24px',
							marginBottom: '32px',
						},
						children: [
							{
								type: 'svg',
								props: {
									viewBox: '0 0 100 100',
									width: 72,
									height: 72,
									children: [
										{
											type: 'polygon',
											props: {
												points:
													'50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5',
												fill: 'none',
												stroke: 'white',
												'stroke-width': '5',
											},
										},
										{
											type: 'clipPath',
											props: {
												id: 'h',
												children: {
													type: 'polygon',
													props: {
														points:
															'50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5',
													},
												},
											},
										},
										{
											type: 'g',
											props: {
												'clip-path': 'url(#h)',
												children: {
													type: 'path',
													props: {
														d: 'M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110',
														stroke: 'white',
														'stroke-width': '8',
														'stroke-linecap': 'round',
														fill: 'none',
													},
												},
											},
										},
									],
								},
							},
							{
								type: 'span',
								props: {
									style: {
										fontSize: 64,
										fontWeight: 700,
										color: 'white',
										letterSpacing: '-0.02em',
									},
									children: 'simse',
								},
							},
						],
					},
				},
				{
					type: 'div',
					props: {
						style: {
							display: 'flex',
							flexDirection: 'column',
							gap: '16px',
						},
						children: [
							{
								type: 'span',
								props: {
									style: {
										fontSize: 36,
										color: '#a1a1aa',
										lineHeight: 1.4,
									},
									children: 'The assistant that ',
								},
							},
						],
					},
				},
				{
					type: 'div',
					props: {
						style: {
							display: 'flex',
							gap: '8px',
							marginTop: '-8px',
						},
						children: [
							{
								type: 'span',
								props: {
									style: {
										fontSize: 36,
										color: '#34d399',
										fontWeight: 700,
									},
									children: 'evolves',
								},
							},
							{
								type: 'span',
								props: {
									style: {
										fontSize: 36,
										color: '#a1a1aa',
									},
									children: ' with you',
								},
							},
						],
					},
				},
				{
					type: 'div',
					props: {
						style: {
							display: 'flex',
							marginTop: '48px',
							height: '3px',
							width: '200px',
							background: 'linear-gradient(to right, #34d399, transparent)',
							borderRadius: '2px',
						},
						children: [],
					},
				},
			],
		},
	},
	{
		width: 1200,
		height: 630,
		fonts: [
			{
				name: 'DM Sans',
				data: fontData,
				weight: 400,
				style: 'normal' as const,
			},
			{
				name: 'DM Sans',
				data: fontData,
				weight: 700,
				style: 'normal' as const,
			},
		],
	},
);

const resvg = new Resvg(svg, {
	fitTo: { mode: 'width', value: 1200 },
});
const png = resvg.render().asPng();

await writeFile(new URL('../public/og-image.png', import.meta.url), png);
console.log('Generated public/og-image.png (1200x630)');
