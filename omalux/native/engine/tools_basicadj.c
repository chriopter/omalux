// SPDX-License-Identifier: GPL-3.0-or-later
// basics adjustments (deprecated): its auto exposure and middle gray picker, ported from
// darktable 5.6.1 without the GTK code. The auto exposure maths (basicadj.c lines 676-1210,
// itself derived from RawTherapee) is copied verbatim with prefixed names.
#include "module_tools_internal.h"
#include "common/colorspaces_inline_conversions.h"

// clang-format off
static inline int64_t basicadj_doubleToRawLongBits(double d)
{
  union {
    double f;
    int64_t i;
  } tmp;
  tmp.f = d;
  return tmp.i;
}

static inline double basicadj_longBitsToDouble(int64_t i)
{
  union {
    double f;
    int64_t i;
  } tmp;
  tmp.i = i;
  return tmp.f;
}

static inline int basicadj_ilogbp1(double d)
{
  const int m = d < 4.9090934652977266E-91;
  d = m ? 2.037035976334486E90 * d : d;
  int q = (basicadj_doubleToRawLongBits(d) >> 52) & 0x7ff;
  q = m ? q - (300 + 0x03fe) : q - 0x03fe;
  return q;
}

// calculate  x * 2^q
static inline double basicadj_ldexpk(double x, int32_t q)
{
  int32_t m = q < 0 ? -1 : 0;
  m = (((m + q) >> 9) - m) << 7;
  q = q - (m << 2);
  double u = basicadj_longBitsToDouble(((int64_t)(m + 0x3ff)) << 52);
  double u2 = u * u;
  u2 = u2 * u2;
  x = x * u2;
  u = basicadj_longBitsToDouble(((int64_t)(q + 0x3ff)) << 52);
  return x * u;
}

static inline double basicadj_xlog(double d)
{
  // since this is a local function and we know that basicadj_xlog will only be
  // called with values 1 <= d <= 65537, there is no need to check for
  // d == INFINITY or d <= 0 and return +/-INFINITY or NAN.

  const int e = basicadj_ilogbp1(d * 0.7071);
  const double m = basicadj_ldexpk(d, -e);

  double x = (m - 1) / (m + 1);
  const double x2 = x * x;

  double t = 0.148197055177935105296783;
  t = fma(t, x2, 0.153108178020442575739679);
  t = fma(t, x2, 0.181837339521549679055568);
  t = fma(t, x2, 0.22222194152736701733275);
  t = fma(t, x2, 0.285714288030134544449368);
  t = fma(t, x2, 0.399999999989941956712869);
  t = fma(t, x2, 0.666666666666685503450651);
  t = fma(t, x2, 2);

  x = x * t + 0.693147180559945286226764 * e;
  return x;
}

static inline double basicadj_gamma2(double x)
{
  const double sRGBGammaCurve = 2.4;
  return (x <= 0.00304) ? (x * 12.92) : (1.055 * exp(log(x) / sRGBGammaCurve) - 0.055);
}

static inline double basicadj_igamma2(double x)
{
  const double sRGBGammaCurve = 2.4;
  return (x <= 0.03928) ? (x / 12.92) : (exp(log((x + 0.055) / 1.055) * sRGBGammaCurve));
}

static void basicadj_get_auto_exp_histogram(const float *const img, const int width, const int height, int *box_area,
                                    uint32_t **_histogram, unsigned int *_hist_size, int *_histcompr)
{
  const int ch = 4;
  const int histcompr = 3;
  const unsigned int hist_size = 65536 >> histcompr;
  uint32_t *histogram = NULL;
  const float mul = hist_size;

  histogram = dt_alloc_align_type(uint32_t, hist_size);
  if(histogram == NULL) goto cleanup;

  memset(histogram, 0, sizeof(uint32_t) * hist_size);

  if(box_area[2] > box_area[0] && box_area[3] > box_area[1])
  {
    for(int y = box_area[1]; y <= box_area[3]; y++)
    {
      const float *const in = img + (size_t)ch * width * y;
      for(int x = box_area[0]; x <= box_area[2]; x++)
      {
        const float *const pixel = in + x * ch;

        for(int c = 0; c < 3; c++)
        {
          if(pixel[c] <= 0.f)
          {
            histogram[0]++;
          }
          else if(pixel[c] >= 1.f)
          {
            histogram[hist_size - 1]++;
          }
          else
          {
            const uint32_t R = (uint32_t)(pixel[c] * mul);
            histogram[R]++;
          }
        }
      }
    }
  }
  else
  {
    for(int i = 0; i < width * height * ch; i += ch)
    {
      const float *const pixel = img + i;

      for(int c = 0; c < 3; c++)
      {
        if(pixel[c] <= 0.f)
        {
          histogram[0]++;
        }
        else if(pixel[c] >= 1.f)
        {
          histogram[hist_size - 1]++;
        }
        else
        {
          const uint32_t R = (uint32_t)(pixel[c] * mul);
          histogram[R]++;
        }
      }
    }
  }

cleanup:
  *_histogram = histogram;
  *_hist_size = hist_size;
  *_histcompr = histcompr;
}

static void basicadj_get_sum_and_average(const uint32_t *const histogram, const int hist_size, float *_sum, float *_avg)
{
  float sum = 0.f;
  float avg = 0.f;

  for(int i = 0; i < hist_size; i++)
  {
    float val = histogram[i];
    sum += val;
    avg += i * val;
  }

  avg /= sum;

  *_sum = sum;
  *_avg = avg;
}

G_GNUC_UNUSED static inline float basicadj_hlcurve(const float level, const float hlcomp, const float hlrange)
{
  if(hlcomp > 0.0f)
  {
    float val = level + (hlrange - 1.f);

    // to avoid division by zero
    if(val == 0.0f)
    {
      val = 0.000001f;
    }

    float Y = val / hlrange;
    Y *= hlcomp;

    // to avoid log(<=0)
    if(Y <= -1.0f)
    {
      Y = -.999999f;
    }

    float R = hlrange / (val * hlcomp);
    return log1pf(Y) * R;
  }
  else
  {
    return 1.f;
  }
}

static void basicadj_get_auto_exp(const uint32_t *const histogram, const unsigned int hist_size, const int histcompr,
                          const float defgain, const float clip, const float midgray, float *_expcomp,
                          float *_bright, float *_contr, float *_black, float *_hlcompr, float *_hlcomprthresh)
{
  float expcomp = 0.f;
  float black = 0.f;
  float bright = 0.f;
  float contr = 0.f;
  float hlcompr = 0.f;
  float hlcomprthresh = 0.f;

  float scale = 65536.0f;

  const int imax = 65536 >> histcompr;
  int overex = 0;
  float sum = 0.f, hisum = 0.f, losum = 0.f;
  float ave = 0.f;

  // find average luminance
  basicadj_get_sum_and_average(histogram, hist_size, &sum, &ave);

  // find median of luminance
  int median = 0, count = histogram[0];

  while(count < sum / 2)
  {
    median++;
    count += histogram[median];
  }

  if(median == 0 || ave < 1.f) // probably the image is a blackframe
  {
    expcomp = 0.f;
    black = 0.f;
    bright = 0.f;
    contr = 0.f;
    hlcompr = 0.f;
    hlcomprthresh = 0.f;
    goto cleanup;
  }

  // compute std dev on the high and low side of median
  // and octiles of histogram
  float octile[8] = { 0.f, 0.f, 0.f, 0.f, 0.f, 0.f, 0.f, 0.f }, ospread = 0.f;
  count = 0;

  int i = 0;

  for(; i < MIN((int)ave, imax); i++)
  {
    if(count < 8)
    {
      octile[count] += histogram[i];

      if(octile[count] > sum / 8.f || (count == 7 && octile[count] > sum / 16.f))
      {
        octile[count] = basicadj_xlog(1. + (float)i) / log(2.f);
        count++;
      }
    }

    losum += histogram[i];
  }

  for(; i < imax; i++)
  {
    if(count < 8)
    {
      octile[count] += histogram[i];

      if(octile[count] > sum / 8.f || (count == 7 && octile[count] > sum / 16.f))
      {
        octile[count] = basicadj_xlog(1.f + (float)i) / log(2.f);
        count++;
      }
    }

    hisum += histogram[i];
  }

  // probably the image is a blackframe
  if(losum == 0.f || hisum == 0.f)
  {
    expcomp = 0.f;
    black = 0.f;
    bright = 0.f;
    contr = 0.f;
    hlcompr = 0.f;
    hlcomprthresh = 0.f;
    goto cleanup;
  }

  // if very overxposed image
  if(octile[6] > log1pf((float)imax) / log2(2.f))  //*** Is this correct?  log2(2) == 1
  {
    octile[6] = 1.5f * octile[5] - 0.5f * octile[4];
    overex = 2;
  }

  // if overexposed
  if(octile[7] > log1pf((float)imax) / log2(2.f))  //*** Is this correct?  log2(2) == 1
  {
    octile[7] = 1.5f * octile[6] - 0.5f * octile[5];
    overex = 1;
  }

  // store values of octile[6] and octile[7] for calculation of exposure compensation
  // if we don't do this and the pixture is underexposed, calculation of exposure compensation assumes
  // that it's overexposed and calculates the wrong direction
  float oct6, oct7;
  oct6 = octile[6];
  oct7 = octile[7];

  for(int ii = 1; ii < 8; ii++)
  {
    if(octile[ii] == 0.0f)
    {
      octile[ii] = octile[ii - 1];
    }
  }

  // compute weighted average separation of octiles
  // for future use in contrast setting
  for(int ii = 1; ii < 6; ii++)
  {
    ospread += (octile[ii + 1] - octile[ii])
               / MAX(0.5f, (ii > 2 ? (octile[ii + 1] - octile[3]) : (octile[3] - octile[ii])));
  }

  ospread /= 5.f;

  // probably the image is a blackframe
  if(ospread <= 0.f)
  {
    expcomp = 0.f;
    black = 0.f;
    bright = 0.f;
    contr = 0.f;
    hlcompr = 0.f;
    hlcomprthresh = 0.f;
    goto cleanup;
  }

  // compute clipping points based on the original histograms (linear, without exp comp.)
  unsigned int clipped = 0;
  int rawmax = (imax)-1;

  while(histogram[rawmax] + clipped <= 0 && rawmax > 1)
  {
    clipped += histogram[rawmax];
    rawmax--;
  }

  // compute clipped white point
  unsigned int clippable = (int)(sum * clip);
  clipped = 0;
  int whiteclip = (imax)-1;

  while(whiteclip > 1 && (histogram[whiteclip] + clipped) <= clippable)
  {
    clipped += histogram[whiteclip];
    whiteclip--;
  }

  // compute clipped black point
  clipped = 0;
  int shc = 0;

  while(shc < whiteclip - 1 && histogram[shc] + clipped <= clippable)
  {
    clipped += histogram[shc];
    shc++;
  }

  // rescale to 65535 max
  rawmax <<= histcompr;
  whiteclip <<= histcompr;
  ave = ave * (1 << histcompr);
  median <<= histcompr;
  shc <<= histcompr;

  // compute exposure compensation as geometric mean of the amount that
  // sets the mean or median at middle gray, and the amount that sets the estimated top
  // of the histogram at or near clipping.
  const float expcomp1 = (logf(midgray * scale / (ave - shc + midgray * shc))) / M_LN2f;
  float expcomp2;

  if(overex == 0) // image is not overexposed
  {
    expcomp2 = 0.5f * ((15.5f - histcompr - (2.f * oct7 - oct6)) + logf(scale / rawmax) / M_LN2f);
  }
  else
  {
    expcomp2 = 0.5f * ((15.5f - histcompr - (2.f * octile[7] - octile[6])) + logf(scale / rawmax) / M_LN2f);
  }

  if(fabsf(expcomp1) - fabsf(expcomp2) > 1.f) // for great expcomp
  {
    expcomp = (expcomp1 * fabsf(expcomp2) + expcomp2 * fabsf(expcomp1)) / (fabsf(expcomp1) + fabsf(expcomp2));
  }
  else
  {
    expcomp = 0.5 * (double)expcomp1 + 0.5 * (double)expcomp2; // for small expcomp
  }

  const float gain = expf(expcomp * M_LN2f);

  const float corr = sqrtf(gain * scale / rawmax);
  black = shc * corr;

  // now tune hlcompr to bring back rawmax to 65535
  hlcomprthresh = 0.f;
  // this is a series approximation of the actual formula for comp,
  // which is a transcendental equation
  const float comp = (gain * ((float)whiteclip) / scale - 1.f) * 2.3f; // 2.3 instead of 2 to increase slightly comp
  hlcompr = (comp / (fmaxf(0.0f, expcomp) + 1.0f));
  hlcompr = fmaxf(0.f, fminf(100.f, hlcompr));

  // now find brightness if gain didn't bring ave to midgray using
  // the envelope of the actual 'control cage' brightness curve for simplicity
  const float midtmp = gain * sqrtf(median * ave) / scale;

  if(midtmp < 0.1f)
  {
    bright = (midgray - midtmp) * 15.0f / (midtmp);
  }
  else
  {
    bright = (midgray - midtmp) * 15.0f / (0.10833 - 0.0833f * midtmp);
  }

  bright = 0.25f * MAX(0.f, bright);

  // compute contrast that spreads the average spacing of octiles
  contr = (midgray * 100.f) * (1.1f - ospread);
  contr = MAX(0.f, MIN(100.f, contr));
  // take gamma into account
  double whiteclipg = basicadj_gamma2(whiteclip * corr);

  float gavg = 0.f;

  float val = 0.f;
  const float increment = corr * (1 << histcompr);

  for(int ii = 0; ii<65536>> histcompr; ii++)
  {
    // gavg += histogram[ii] * _get_LUTf(gamma2curve, gamma2curve_size, val);
    gavg += histogram[ii] * basicadj_gamma2(val);
    val += increment;
  }

  gavg /= sum;

  if(black < gavg)
  {
    const int maxwhiteclip = (gavg - black) * 4 / 3
                             + black; // don't let whiteclip be so large that the histogram average goes above 3/4

    if(whiteclipg < maxwhiteclip)
    {
      whiteclipg = maxwhiteclip;
    }
  }

  whiteclipg
      = basicadj_igamma2(whiteclipg); // need to inverse gamma transform to get correct exposure compensation parameter

  // correction with gamma
  black = (black / whiteclipg);

  expcomp = CLAMP(expcomp, -5.0f, 12.0f);

  bright = MAX(-100.f, MIN(bright, 100.f));

cleanup:
  black /= 100.f;
  bright /= 100.f;
  contr /= 100.f;

  if(dt_isnan(expcomp))
  {
    expcomp = 0.f;
    dt_print(DT_DEBUG_ALWAYS, "[basicadj_get_auto_exp] expcomp is NaN!");
  }
  if(dt_isnan(black))
  {
    black = 0.f;
    dt_print(DT_DEBUG_ALWAYS, "[basicadj_get_auto_exp] black is NaN!");
  }
  if(dt_isnan(bright))
  {
    bright = 0.f;
    dt_print(DT_DEBUG_ALWAYS, "[basicadj_get_auto_exp] bright is NaN!");
  }
  if(dt_isnan(contr))
  {
    contr = 0.f;
    dt_print(DT_DEBUG_ALWAYS, "[basicadj_get_auto_exp] contr is NaN!");
  }
  if(dt_isnan(hlcompr))
  {
    hlcompr = 0.f;
    dt_print(DT_DEBUG_ALWAYS, "[basicadj_get_auto_exp] hlcompr is NaN!");
  }
  if(dt_isnan(hlcomprthresh))
  {
    hlcomprthresh = 0.f;
    dt_print(DT_DEBUG_ALWAYS, "[basicadj_get_auto_exp] hlcomprthresh is NaN!");
  }

  *_expcomp = expcomp;
  *_black = black;
  *_bright = bright;
  *_contr = contr;
  *_hlcompr = hlcompr;
  *_hlcomprthresh = hlcomprthresh;
}

static void basicadj_auto_exposure(const float *const img, const int width, const int height, int *box_area,
                           const float clip, const float midgray, float *_expcomp, float *_bright, float *_contr,
                           float *_black, float *_hlcompr, float *_hlcomprthresh)
{
  uint32_t *histogram = NULL;
  unsigned int hist_size = 0;
  int histcompr = 0;

  const float defGain = 0.0f;

  basicadj_get_auto_exp_histogram(img, width, height, box_area, &histogram, &hist_size, &histcompr);
  basicadj_get_auto_exp(histogram, hist_size, histcompr, defGain, clip, midgray, _expcomp, _bright, _contr, _black,
                _hlcompr, _hlcomprthresh);

  if(histogram) dt_free_align(histogram);
}
// clang-format on

#define P(module, name) ((float *)(module)->get_p((module)->params, (name)))

// basicadj.c process (lines 1296-1333) as the "auto" button (whole image) or "select region"
// (the drawn box, _get_selected_area) requests it, on the module input.
static int basicadj_auto_box(OmToolContext *ctx, gboolean region) {
    dt_iop_module_t *m = ctx->module;
    OmCapture *c = ctx->capture;
    if (!m->get_f("hlcomprthresh") || !c || c->dsc.channels != 4)
        return 3;
    int box[4] = {0};
    if (region && c->box_valid) {
        box[0] = MIN(c->roi.width - 1, c->box[0]);
        box[1] = MIN(c->roi.height - 1, c->box[1]);
        box[2] = MIN(c->roi.width - 1, c->box[2]);
        box[3] = MIN(c->roi.height - 1, c->box[3]);
        if (box[2] - box[0] < 1 || box[3] - box[1] < 1)
            box[0] = box[1] = box[2] = box[3] = 0;
    }
    basicadj_auto_exposure(c->input, c->roi.width, c->roi.height, box, *P(m, "clip"), *P(m, "middle_grey") / 100.f,
                           P(m, "exposure"), P(m, "brightness"), P(m, "contrast"), P(m, "black_point"),
                           P(m, "hlcompr"), P(m, "hlcomprthresh"));
    return 0;
}
static int basicadj_auto(OmToolContext *ctx) {
    return basicadj_auto_box(ctx, FALSE);
}
static int basicadj_region(OmToolContext *ctx) {
    return basicadj_auto_box(ctx, TRUE);
}

// basicadj.c color_picker_apply (lines 482-500): middle gray from the picked luminance.
static int basicadj_middle_grey(OmToolContext *ctx) {
    dt_iop_module_t *m = ctx->module;
    if (!m->get_f("middle_grey"))
        return 3;
    const dt_iop_order_iccprofile_info_t *const work_profile =
        dt_ioppr_get_pipe_current_profile_info(m, &ctx->capture->pipe);
    dt_aligned_pixel_t picked;
    copy_pixel(picked, ctx->picked.in[DT_PICK_MEAN]);
    *P(m, "middle_grey") =
        work_profile ? dt_ioppr_get_rgb_matrix_luminance(picked, work_profile->matrix_in, work_profile->lut_in,
                                                         work_profile->unbounded_coeffs_in, work_profile->lutsize,
                                                         work_profile->nonlinearlut) * 100.f
                     : dt_camera_rgb_luminance(picked);
    return 0;
}

const OmToolSpec om_tools_basicadj[] = {
    // basicadj.c:653 auto, :656 select region, :637 the middle gray slider's picker
    {"basicadj", "auto", OM_TOOL_BUTTON, -1, OM_TOOL_CAPTURE, basicadj_auto},
    {"basicadj", "select_region", OM_TOOL_AREA, -1, 0, basicadj_region},
    {"basicadj", "middle_grey", OM_TOOL_AREA, -1, 0, basicadj_middle_grey},
    {NULL, NULL, 0, 0, 0, NULL},
};
