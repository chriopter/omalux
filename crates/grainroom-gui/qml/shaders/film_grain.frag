#version 440

// SPDX-License-Identifier: GPL-3.0-or-later
// Film grain model adapted for the GPU from darktable src/iop/grain.c and
// RawTherapee rtengine/ipgrain.cc. See docs/grain-model.md for provenance.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float grainAmount;
    vec2 imageSize;
    float grainCoarseness;
    float midtonesBias;
    float grainSeed;
} ubuf;

layout(binding = 1) uniform sampler2D source;

// Compact GPU simplex noise based on the public-domain algorithm by
// Stefan Gustavson and the MIT-licensed Ashima Arts GLSL formulation.
vec3 permute(vec3 x) {
    return mod(((x * 34.0) + 1.0) * x, 289.0);
}

float simplexNoise(vec2 point) {
    const vec4 c = vec4(
        0.211324865405187,
        0.366025403784439,
       -0.577350269189626,
        0.024390243902439
    );

    vec2 cell = floor(point + dot(point, c.yy));
    vec2 local0 = point - cell + dot(cell, c.xx);
    vec2 corner = local0.x > local0.y ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    vec4 local12 = local0.xyxy + c.xxzz;
    local12.xy -= corner;

    cell = mod(cell, 289.0);
    vec3 permutation = permute(
        permute(cell.y + vec3(0.0, corner.y, 1.0))
        + cell.x + vec3(0.0, corner.x, 1.0)
    );

    vec3 weight = max(
        0.5 - vec3(
            dot(local0, local0),
            dot(local12.xy, local12.xy),
            dot(local12.zw, local12.zw)
        ),
        0.0
    );
    weight *= weight;
    weight *= weight;

    vec3 gradientX = 2.0 * fract(permutation * c.www) - 1.0;
    vec3 gradientH = abs(gradientX) - 0.5;
    vec3 gradientOffset = floor(gradientX + 0.5);
    vec3 gradient = gradientX - gradientOffset;
    weight *= 1.79284291400159
        - 0.85373472095314 * (gradient * gradient + gradientH * gradientH);

    vec3 contribution;
    contribution.x = gradient.x * local0.x + gradientH.x * local0.y;
    contribution.yz = gradient.yz * local12.xz + gradientH.yz * local12.yw;
    return 130.0 * dot(weight, contribution);
}

float filmNoise(vec2 normalizedPosition, float scale, float seed) {
    // Frequencies and amplitudes are darktable's fit to real grain scans.
    const vec3 frequency = vec3(0.4910, 0.9441, 1.7280);
    const vec3 amplitude = vec3(0.2340, 0.7850, 1.2150);

    float total = 0.0;
    total += simplexNoise(normalizedPosition * frequency.x / scale + vec2(seed, 0.0)) * amplitude.x;
    total += simplexNoise(normalizedPosition * frequency.y / scale + vec2(seed, 17.0)) * amplitude.y;
    total += simplexNoise(normalizedPosition * frequency.z / scale + vec2(seed, 37.0)) * amplitude.z;
    return total;
}

float paperDelta(float bias) {
    return 2.0 * exp(clamp(bias, 0.0, 1.0) * log(0.0001));
}

float paperResponse(float exposure, float bias) {
    float delta = paperDelta(bias);
    return (1.0 + 2.0 * delta)
        / (1.0 + exp((4.0 * (0.5 - exposure)) / (1.0 + 2.0 * delta)))
        - delta;
}

float inversePaperResponse(float density, float bias) {
    float delta = paperDelta(bias);
    float safeDensity = clamp(density, 0.00001, 0.99999);
    return -log((1.0 + 2.0 * delta) / (safeDensity + delta) - 1.0)
        * (1.0 + 2.0 * delta) / 4.0 + 0.5;
}

void main() {
    vec4 pixel = texture(source, qt_TexCoord0);
    vec2 safeSize = max(ubuf.imageSize, vec2(1.0));
    float shortEdge = max(min(safeSize.x, safeSize.y), 1.0);
    vec2 imagePosition = qt_TexCoord0 * safeSize / shortEdge;

    // darktable exposes this range as an approximate film ISO (20–6400).
    float grainScale = (1.0 + clamp(ubuf.grainCoarseness, 20.0, 6400.0) / 2665.0) / 800.0;
    float noise = filmNoise(imagePosition, grainScale, ubuf.grainSeed);

    // Procedural noise cannot be texture-filtered. Fade it by the source-pixel
    // footprint when zoomed out to avoid the inaccurate, aliased preview that
    // older RawTherapee versions exhibited.
    vec2 sourceFootprint = fwidth(qt_TexCoord0) * safeSize;
    float previewFilter = 1.0 / max(1.0, max(sourceFootprint.x, sourceFootprint.y));
    noise *= previewFilter;

    float luminance = dot(pixel.rgb, vec3(0.2126, 0.7152, 0.0722));
    float noisyExposure = inversePaperResponse(luminance, ubuf.midtonesBias)
        + noise * ubuf.grainAmount * 0.15;
    float developedLuminance = paperResponse(noisyExposure, ubuf.midtonesBias);

    // darktable modifies Lab lightness only. Equal RGB displacement is the
    // display-RGB equivalent that preserves the existing channel differences.
    vec3 developed = clamp(pixel.rgb + vec3(developedLuminance - luminance), 0.0, 1.0);
    fragColor = vec4(developed, pixel.a) * ubuf.qt_Opacity;
}
